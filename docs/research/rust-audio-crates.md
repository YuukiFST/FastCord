# Rust audio crates for capture, Opus and voice processing on Windows

Research for [issue #10](https://github.com/YuukiFST/FastCord/issues/10).
All versions and dates below were read from the crates.io API and from each project's own repository on 2026-09-21, not from memory.

## The question

FastCord needs a voice pipeline on Windows: device enumeration, capture and playback, Opus encode and decode at Discord's parameters, voice activity detection, and optionally noise suppression, echo cancellation and automatic gain control.
Low CPU is a hard goal, and every added C or C++ dependency is a Windows build-toolchain liability, so the evaluation weighs "what does a contributor have to install before `cargo build` works" as heavily as it weighs features.

## Capture and playback

`cpal` is the only realistic option and it is in good shape.
Version 0.18.2 was published on 2026-08-16, the crate is maintained under the RustAudio organisation, and it has around 6.1 million recent downloads.
On Windows its default backend is WASAPI, with ASIO and JACK as optional backends, and the README lists Windows 10 as the minimum supported OS for the WASAPI backend with an MSRV of 1.85.
It needs no external build tooling on Windows: the WASAPI backend binds through the `windows` crate, so there is no cmake, no clang and no vendored C.
The license is Apache-2.0 (it was already Apache-2.0 as of 0.17.0 in December 2025), which is worth noting because older material describes cpal as dual MIT/Apache.

cpal covers what the ticket asks of it: it enumerates hosts and devices, looks devices up by stable ID or by default input/output role, exposes device metadata, and builds input and output streams with either compile-time or runtime sample formats.
That is enough for a device picker in settings and for remembering a chosen microphone across restarts.

The thing cpal does not give us is a clock relationship between the capture callback and our encoder.
WASAPI hands us whatever buffer size it wants, while Opus for Discord wants exactly 20 ms frames, so a lock-free ring buffer sits between the two.
`ringbuf` 0.5.2 (2026-09-13, MIT OR Apache-2.0, pure Rust) is the standard choice and songbird itself depends on the same crate family.

## Opus

There are two live safe wrappers and they differ mainly in which `-sys` crate they pull.

`opus` 0.4.0 was published on 2026-08-23 by SpaceManiac, is dual MIT/Apache-2.0, and depends on `opusic-sys` 0.7.x.
`opusic-sys` 0.7.5 (2026-08-06, BSD-3-Clause) targets libopus 1.6.1, bundles the C source by default, and its own CI matrix includes `windows-latest`, so the MSVC path is exercised on every push rather than assumed.
Its documented requirement is cmake when building with the `bundled` feature, optionally ninja if present, and `LIBCLANG_PATH` only if you regenerate bindings with the `build-bindgen` feature, which we would not.
So the real Windows prerequisite is cmake plus the MSVC C toolchain the user already needs for Rust on Windows.

`opus2` 0.4.0 (2026-03-26, MIT/Apache-2.0) is a fork of the same code that songbird 0.6.0 depends on.
Its default `backend-libopus` routes through `libopus_sys` 0.4.0 and wants either pkg-config with opus headers, or cmake, make and a C compiler.
Its interesting feature is `backend-mousiki`, which swaps in `mousiki`, a pure-Rust Opus codec by the same author.
`mousiki` 0.2.1 was published on 2026-03-26, has roughly 1,100 recent downloads and no README in its repository, so it is far too young to carry FastCord's only codec path.
It is worth tracking as a future escape hatch from the cmake requirement, not as a v1 choice.

`audiopus` is the crate most older Discord-in-Rust material points at, and it is dead: the last stable release is 0.2.0 from 2019-10-11, the last publish of any kind is the 0.3.0-rc.0 pre-release from 2021-04-22, and `audiopus_sys` was last touched the same day.
It still gets a million downloads a quarter purely from old lockfiles.
`opus-sys` is worse, last published 2016.
Neither should be used.

### What songbird does, as a reference implementation

songbird 0.6.0 (2026-04-05, ISC) is Discord's de facto Rust voice library and its constants file is the cleanest statement of the parameters we must match.
It encodes at 48 kHz, 50 frames per second, so a 20 ms frame of 960 samples per channel and 1920 interleaved samples in stereo, with a default bitrate of 128 kbit/s and a maximum voice packet size of 1460 bytes chosen to stay under the Ethernet MTU.
The mixer builds its encoder with `Encoder::new(48000, mix_mode.to_opus(), Application::Audio)` and applies `SoftClip` before encoding.
It also supports Opus passthrough, forwarding already-encoded frames unchanged, and gives up on passthrough after three bad frames.

Two caveats on using songbird as a model.
It is a bot library, so it has no capture side at all: it mixes decoded sources and sends, and its input side is symphonia-based file and stream decoding, not a microphone.
And its `Application::Audio` coding mode is right for music playback but wrong for us; a speech client should use the voice coding mode, and should enable Opus in-band FEC and DTX, which songbird has no reason to touch.
Resampling in songbird is handled by `rubato`, which is also the crate we want for the 48 kHz to 16 kHz conversion the VAD needs; `rubato` 5.0.0 shipped 2026-08-10 under MIT OR Apache-2.0.

## Voice activity detection

`earshot` 1.2.2 (2026-08-19, MIT OR Apache-2.0) is the strongest option and it is pure Rust.
It operates on 16 ms frames of 16 kHz audio, exactly 256 samples per frame, supports streaming and both mono and interleaved stereo, and its author reports a real-time factor of 0.0003 against Silero VAD v6 and TEN VAD.
The numbers that matter more for us are the footprint ones: roughly 8 KiB of state per detector instance and about 95 KiB of binary, against Silero's 2 MiB model plus roughly 8 MB of ONNX Runtime.
It is `no_std`-capable and needs no build tooling whatsoever.

`voice_activity_detector` 0.2.1 (2025-08-04) wraps Silero VAD v5 and is a perfectly good crate, but it drags ONNX Runtime into a desktop client that otherwise has no ML runtime, which is the opposite of a low-CPU, small-binary goal.

`webrtc-vad` 0.4.0 is unmaintained, last published 2019-10-01, and its build script compiles libfvad from a git submodule with `cc` - meaning it shells out to `git submodule update --init` at build time, which is not a dependency model that survives a clean crates.io install.
Skip it.

## Noise suppression

`nnnoiseless` 0.5.2 (2025-12-18, BSD-3-Clause) is a safe Rust port of Xiph's RNNoise, with no C dependency at all: its runtime dependencies are `easyfft` and `once_cell`, and everything else is optional or dev-only.
It works on 48 kHz audio in 480-sample frames, which lines up neatly with the 960-sample Opus frame, two nnnoiseless frames per packet.
On cost, the upstream RNNoise author reports the unoptimised C running about 60x faster than real time on an x86 CPU and about 7x faster than real time on a Raspberry Pi 3, so on a desktop this is low single-digit percent of one core.
The release cadence is slow, 0.5.1 in 2022 then 0.5.2 in 2025, but that is a stable port of a finished algorithm rather than an abandoned project.

`rnnoise-c` 0.2.1 is effectively dead: last published 2020-07-13, with 14 recent downloads.
It offers nothing `nnnoiseless` does not, and costs a C dependency.

## Echo cancellation and AGC

`webrtc-audio-processing` 2.1.0 (2026-05-13) wraps PulseAudio's repackaging of WebRTC's AudioProcessing module and is the only crate that offers real AEC3, plus noise suppression, AGC and its own VAD in one place.
It is actively maintained by tonarino.
It is also the one crate here that we cannot ship on Windows in the first version.

The `bundled` feature builds the vendored C++ and requires clang or gcc, pkg-config, meson and ninja-build, and the vendored WebRTC source lives in a git submodule.
Without `bundled`, the build tries to dynamically link a library installed through the OS package manager, with `libwebrtc-audio-processing-dev` on Debian and a pacman package on Arch - there is no Windows equivalent.
The project's CI matrix runs `ubuntu-latest` and `macos-latest` only, with no Windows job at all.
Issue #34, "Windows build", has been open since 2023-09-27 and was last touched 2026-08-08 with no resolution.
The license is flagged non-standard on crates.io because it inherits the BSD-3-Clause-plus-patent terms of the WebRTC source, which is permissive but needs a real read before shipping.

Treating this as "blocked on Windows" rather than "hard on Windows" is the honest reading of the evidence.

## Comparison

| Crate | Latest | Date | Maintained | Windows build needs | License | CPU |
|---|---|---|---|---|---|---|
| `cpal` | 0.18.2 | 2026-08-16 | yes, active | none (WASAPI via `windows` crate) | Apache-2.0 | negligible |
| `opus` | 0.4.0 | 2026-08-23 | yes | cmake + MSVC C compiler; Windows in `opusic-sys` CI | MIT/Apache-2.0 | low, ~1-2% core per stream |
| `opus2` | 0.4.0 | 2026-03-26 | yes | cmake + make + C compiler, or pkg-config | MIT/Apache-2.0 | same as above |
| `mousiki` | 0.2.1 | 2026-03-26 | young, ~1.1k recent dl | none (pure Rust) | MIT | unmeasured |
| `audiopus` | 0.2.0 | 2019-10-11 | no | cmake/autotools | ISC | n/a |
| `opus-sys` | 0.2.1 | 2016-07-04 | no | n/a | MIT | n/a |
| `nnnoiseless` | 0.5.2 | 2025-12-18 | slow but alive | none (pure Rust) | BSD-3-Clause | ~60x real time upstream |
| `rnnoise-c` | 0.2.1 | 2020-07-13 | no | C compiler | MIT OR Apache-2.0 | n/a |
| `webrtc-audio-processing` | 2.1.0 | 2026-05-13 | yes, but no Windows CI | clang, pkg-config, meson, ninja, git submodule | non-standard (WebRTC BSD+patent) | moderate (AEC3) |
| `earshot` | 1.2.2 | 2026-08-19 | yes, active | none (pure Rust, `no_std`-capable) | MIT OR Apache-2.0 | RTF 0.0003 |
| `voice_activity_detector` | 0.2.1 | 2025-08-04 | yes | none, but pulls ONNX Runtime | non-standard | higher, ~8 MB binary cost |
| `webrtc-vad` | 0.4.0 | 2019-10-01 | no | C compiler + git submodule at build time | MIT | low |
| `rubato` | 5.0.0 | 2026-08-10 | yes | none | MIT OR Apache-2.0 | low |
| `ringbuf` | 0.5.2 | 2026-09-13 | yes | none | MIT OR Apache-2.0 | negligible |
| `songbird` | 0.6.0 | 2026-04-05 | yes | inherits `opus2` | ISC | reference only |

## Recommended crate set

For the first version:

- `cpal` 0.18 for device enumeration, capture and playback on WASAPI.
- `ringbuf` 0.5 between the WASAPI callback and the encoder thread, so the callback never blocks.
- `rubato` 5 for the 48 kHz to 16 kHz downmix feeding the VAD.
- `opus` 0.4 for encode and decode, at 48 kHz, 20 ms frames, voice coding mode, with in-band FEC and DTX enabled.
- `earshot` 1.2 for voice activity detection at 16 kHz on 256-sample frames.
- `nnnoiseless` 0.5 for optional noise suppression at 48 kHz on 480-sample frames.

The only non-Rust build prerequisite this set introduces is cmake, from the Opus bindings.
That needs to go in the README's build instructions and in CI, and it is the single thing standing between a contributor and a clean `cargo build` on a fresh Windows machine.

Push-to-talk needs none of this: it is a keyboard hook gating the encoder, and it should be the default mode precisely because it costs nothing.

## Viable in the first version

- Device enumeration and selection, capture and playback, through cpal on WASAPI.
- Opus encode and decode at Discord's parameters, with FEC and DTX.
- Push-to-talk.
- Voice activation, through `earshot`, with a user-adjustable threshold and a hold-time so the tail of a word is not cut.
- Noise suppression, through `nnnoiseless`, off by default and toggleable in settings.

## Not viable in the first version

- Acoustic echo cancellation. The only crate that implements it properly does not build on Windows, and writing an AEC by hand is a research project, not a ticket. Headphone users do not need it, so v1 should say so plainly in the settings UI rather than ship a checkbox that does nothing.
- Automatic gain control, at least the WebRTC one, for the same reason. A simple RMS-target gain with a limiter is a plausible in-house substitute if it turns out to matter, but it is not the same feature and should not be labelled as if it were.
- A pure-Rust Opus path via `mousiki`, which would remove the cmake requirement. Revisit when it has a README, a changelog and more than four figures of downloads.

## Sources

- cpal on crates.io: https://crates.io/crates/cpal
- cpal README (platform table, MSRV, backends): https://github.com/RustAudio/cpal
- opus crate: https://crates.io/crates/opus and https://github.com/SpaceManiac/opus-rs
- opusic-sys README (cmake requirement, libopus 1.6.1, bundling): https://github.com/DoumanAsh/opusic-sys
- opusic-sys CI matrix including windows-latest: https://github.com/DoumanAsh/opusic-sys/blob/master/.github/workflows/rust.yml
- opus2 crate and README: https://github.com/cijiugechu/opus2
- mousiki: https://crates.io/crates/mousiki
- audiopus: https://crates.io/crates/audiopus
- songbird constants (48 kHz, 50 fps, 128 kbit/s, 1460-byte packets): https://github.com/serenity-rs/songbird/blob/current/src/constants.rs
- songbird mixer (encoder construction, SoftClip, passthrough): https://github.com/serenity-rs/songbird/blob/current/src/driver/tasks/mixer/mod.rs
- nnnoiseless: https://crates.io/crates/nnnoiseless and https://github.com/jneem/nnnoiseless
- RNNoise performance claims (60x real time on x86): https://jmvalin.ca/demo/rnnoise/
- webrtc-audio-processing README (build tooling, dynamic linking, packages): https://github.com/tonarino/webrtc-audio-processing
- webrtc-audio-processing CI matrix (no Windows job): https://github.com/tonarino/webrtc-audio-processing/blob/main/.github/workflows/rust.yml
- webrtc-audio-processing issue #34, "Windows build", open since 2023: https://github.com/tonarino/webrtc-audio-processing/issues/34
- earshot README (frame size, RTF, footprint): https://github.com/pykeio/earshot
- voice_activity_detector README: https://github.com/nkeenan38/voice_activity_detector
- webrtc-vad build script (git submodule + cc): https://github.com/kaegi/webrtc-vad/blob/master/build.rs
- rubato: https://crates.io/crates/rubato
- ringbuf: https://crates.io/crates/ringbuf
