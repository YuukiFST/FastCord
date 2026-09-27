# Fixtures do gateway

Estes arquivos (`ready.json`, `ready_supplemental.json`, `capture-meta.json`)
são **sintéticos**: escritos à mão no formato da captura real, só para os
testes rodarem sem segredo nenhum. Todo id é um placeholder
`1000000000000000NN`, todo nome é `synthetic_*`.

## Como trocar pela captura real (issue #45)

1. Copie o token no Discord web (F12 → Network → `discord.com/api/v9/` →
   header `authorization`).
2. Na pasta `FastCord`, rode no PowerShell:

```powershell
$env:FASTCORD_TOKEN = "<cole aqui>"
cargo run --release --manifest-path spikes/gateway-capture/Cargo.toml
Remove-Item Env:FASTCORD_TOKEN
```

3. Converta o que foi gerado em `spikes/gateway-capture/tests/fixtures/`
   para o formato destes arquivos (só o payload `d` do READY e do
   READY_SUPPLEMENTAL) e troque o conteúdo, mantendo os nomes.
4. Nunca cole token, id ou nome real em nenhum arquivo do repo.
