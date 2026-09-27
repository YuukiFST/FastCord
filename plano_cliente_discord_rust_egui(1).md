# Plano de Projeto — Cliente Discord Rápido (Rust + egui)

## 1. Visão geral

Cliente Discord alternativo, nativo, leve e rápido, desenvolvido em Rust com `egui`/`eframe`.

O objetivo é oferecer uma experiência completa para comunicação em comunidades, mensagens diretas e canais de voz, priorizando:

- Inicialização rápida.
- Baixo consumo de RAM.
- Baixo consumo de CPU.
- Interface responsiva.
- Binário nativo.
- Ausência de Electron e Chromium.
- Código modular, testável e open source.
- Ausência de serviços próprios pagos ou infraestrutura obrigatória.
- Foco em funcionalidades relevantes, sem recursos comerciais ou cosméticos desnecessários.

> **Observação de compatibilidade:** a implementação deve respeitar os mecanismos oficiais e os Termos de Serviço do Discord. Não se deve presumir que uma conta de usuário possa ser automatizada por APIs destinadas a bots. O método de autenticação e acesso efetivamente utilizado deverá ser validado antes da implementação.

---

## 2. Objetivos principais

### 2.1 Performance

- Abrir em menos de 1 segundo, quando possível.
- Buscar consumo de RAM próximo ou inferior a aproximadamente 150 MB em uso normal, como meta de referência e não como garantia.
- Manter baixo consumo de CPU em estado ocioso.
- Evitar renderização contínua desnecessária.
- Carregar dados e imagens sob demanda.
- Evitar manter mensagens e recursos que não estão em uso na memória.
- Usar cache com limites configuráveis.
- Manter a interface responsiva mesmo em servidores grandes.
- Reduzir alocações e cópias desnecessárias.
- Medir tempo de inicialização, uso de memória, CPU, latência e tempo de renderização.

### 2.2 Experiência de uso

- Navegação rápida.
- Atalhos de teclado.
- Busca eficiente.
- Interface consistente.
- Feedback claro de conexão, envio, erro e reconexão.
- Configurações acessíveis.
- Suporte a tema claro, escuro e tema do sistema.
- Acessibilidade básica por teclado, contraste e dimensionamento de fonte.

---

## 3. Stack tecnológica

### 3.1 Base

- Rust.
- `egui`.
- `eframe`.
- Cargo.
- `tracing`/`tracing-subscriber` para logging.
- `thiserror` e/ou `anyhow` para tratamento de erros.
- `serde` para serialização.
- `toml` ou formato equivalente para configurações.

### 3.2 Rede

- Cliente HTTP assíncrono.
- WebSocket.
- Runtime assíncrono, como `tokio`, quando necessário.
- Controle de reconexão.
- Controle de rate limits.
- Timeouts e cancelamento de tarefas.

### 3.3 Dados locais

- SQLite ou outro armazenamento local leve.
- Cache de arquivos em disco.
- `keyring` para credenciais.
- Migrações de banco local.
- Limpeza automática de cache.

### 3.4 Áudio

- Biblioteca de captura e reprodução de áudio compatível com os sistemas operacionais suportados.
- Abstração própria para dispositivos de entrada e saída.
- Processamento local de áudio quando viável.
- Separação entre captura, processamento, transmissão e reprodução.

---

## 4. Escopo funcional

# 4.1 Autenticação

## Deve existir

- Login por QR Code.
- Fluxo de autorização por QR Code.
- Exibição de estado do QR Code.
- Expiração e renovação do QR Code.
- Indicação de QR Code expirado.
- Logout.
- Troca de conta.
- Persistência segura da sessão.
- Possibilidade de múltiplas contas locais.
- Tratamento de sessão expirada.
- Mensagens claras para falhas de autenticação.

## Segurança

- Nunca salvar tokens em texto puro em arquivos de configuração.
- Usar armazenamento seguro do sistema operacional:
  - Windows Credential Manager.
  - macOS Keychain.
  - Secret Service/libsecret no Linux.
- Limpar segredos de memória quando possível.
- Não registrar tokens em logs.
- Não incluir credenciais em relatórios de erro.
- Proteger arquivos locais de configuração e cache.
- Permitir apagar todos os dados locais da conta.

---

# 4.2 Gateway e tempo real

## Deve existir

- Conexão WebSocket.
- Handshake.
- Identificação.
- Heartbeat.
- Reconexão automática.
- Retomada de sessão quando suportada.
- Tratamento de desconexões.
- Controle de backoff.
- Estado de conexão visível.
- Processamento de eventos em segundo plano.
- Cancelamento de tarefas ao fechar o aplicativo.

## Eventos relevantes

- Novas mensagens.
- Mensagens editadas.
- Mensagens excluídas.
- Atualização de servidores.
- Atualização de canais.
- Atualização de membros.
- Atualização de cargos.
- Atualização de permissões.
- Alterações de status.
- Atualizações de presença.
- Atualizações de mensagens não lidas.
- Eventos de voz.
- Eventos de conexão e desconexão.

---

# 4.3 API HTTP/REST

## Deve existir

- Obtenção de histórico de mensagens.
- Envio de mensagens.
- Edição de mensagens.
- Exclusão de mensagens.
- Respostas a mensagens.
- Reações.
- Fixação de mensagens.
- Busca de mensagens.
- Upload de arquivos.
- Download de anexos.
- Consulta de servidores.
- Consulta de canais.
- Consulta de membros.
- Consulta de permissões.
- Consulta de threads.
- Tratamento de erros HTTP.
- Tratamento de rate limits.
- Retry apenas quando seguro e apropriado.
- Cancelamento de requisições.
- Cache de respostas que possam ser armazenadas com segurança.

---

# 4.4 Mensagens

## Deve existir

- Mensagens de texto.
- DMs.
- Grupos de DM.
- Canais de texto.
- Threads.
- Replies.
- Menções.
- Markdown básico.
- Negrito.
- Itálico.
- Sublinhado.
- Tachado.
- Código inline.
- Blocos de código.
- Links.
- Embeds.
- Emojis.
- Reações.
- Mensagens fixadas.
- Edição de mensagens.
- Exclusão de mensagens.
- Copiar texto.
- Copiar link da mensagem.
- Responder mensagem.
- Editar mensagem própria.
- Excluir mensagem própria quando permitido.
- Indicador de mensagem editada.
- Indicador de anexos.
- Indicador de mensagens não lidas.
- Separador de mensagens novas.
- Histórico paginado.
- Carregamento incremental.
- Scroll até a primeira mensagem não lida.
- Scroll até a última mensagem.
- Visualização de respostas e threads.

## Comportamento da caixa de texto

- Enter para enviar.
- Shift + Enter para nova linha.
- Ctrl + Enter configurável.
- Histórico local de rascunhos.
- Rascunho por canal ou DM.
- Recuperação de rascunho após troca de canal.
- Autocomplete de menções.
- Autocomplete de emojis.
- Autocomplete de canais.
- Colagem de texto e imagens quando suportada.
- Indicador de envio.
- Reenvio manual em caso de falha.
- Cancelamento de upload quando possível.

---

# 4.5 Servidores, canais e membros

## Deve existir

- Lista de servidores.
- Ícones de servidores.
- Lista de canais.
- Categorias.
- Canais de texto.
- Canais de voz.
- Canais privados.
- Threads.
- Canais favoritos.
- Servidores favoritos.
- Reordenação visual local.
- Lista de membros.
- Busca de membros.
- Status dos usuários.
- Avatares.
- Cargos.
- Permissões.
- Canais silenciados.
- Servidores silenciados.
- Indicadores de atividade.
- Contagem de mensagens não lidas.
- Indicação de menções.
- Collapse/expand de categorias.
- Ocultação de canais sem atividade, quando configurada.
- Visualização de canais inacessíveis ou sem permissão de forma clara.

---

# 4.6 DMs e navegação

## Deve existir

- Lista de DMs recentes.
- Busca de DMs.
- Fixar DMs.
- Fechar/remover DM da lista local sem apagar a conversa.
- Indicadores de mensagens não lidas.
- Histórico de navegação.
- Voltar e avançar entre canais.
- Último canal visitado.
- Quick Switcher.
- Atalho para ir ao canal ou DM anterior.
- Atalho para ir à primeira mensagem não lida.
- Atalho para abrir busca.

## Quick Switcher

Atalho sugerido: `Ctrl + K`.

Permitir pesquisar:

- Servidores.
- Canais.
- DMs.
- Usuários.
- Mensagens.
- Threads.

---

# 4.7 Voz e áudio

O suporte a voz é parte central do projeto.

## Conexão de voz

- Entrar em canais de voz.
- Sair de canais de voz.
- Reconectar em caso de queda.
- Exibir estado da conexão.
- Exibir latência quando disponível.
- Exibir usuários conectados.
- Indicar quem está falando.
- Silenciar o próprio microfone.
- Desativar o próprio áudio.
- Controle de volume individual.
- Ajuste de volume geral.
- Tratamento de falhas de dispositivo.

## Configuração de entrada

- Selecionar dispositivo de entrada.
- Listar microfones disponíveis.
- Atualizar lista de dispositivos.
- Testar microfone.
- Volume de entrada.
- Ganho de entrada quando suportado.
- Sensibilidade do microfone.
- Detecção automática de voz.
- Limite de ativação de voz.
- Push-to-talk.
- Atalho configurável de Push-to-talk.
- Opção de manter o microfone aberto.
- Indicador de nível do microfone.
- Indicador de clipping ou saturação quando possível.

## Configuração de saída

- Selecionar dispositivo de saída.
- Listar dispositivos de saída disponíveis.
- Atualizar lista de dispositivos.
- Testar saída de áudio.
- Volume geral.
- Volume individual dos usuários.
- Silenciar sons do aplicativo.
- Selecionar dispositivo padrão do sistema.
- Tratamento de troca de dispositivo durante uma chamada.

## Processamento de voz

Implementar quando tecnicamente viável:

- Supressão de ruído.
- Cancelamento de eco.
- Controle automático de ganho.
- Filtro de ruído.
- Detecção de voz.
- Ajuste de sensibilidade.
- Redução de ruído constante.
- Configuração de intensidade do processamento.
- Ativação e desativação independente de cada recurso.

Todo processamento local deve priorizar baixo consumo de CPU e permitir desativação.

---

# 4.8 Arquivos, imagens, GIFs e mídia

## Deve existir

- Envio de arquivos.
- Download de arquivos.
- Visualização de imagens.
- Preview de imagens.
- Visualização de vídeos quando suportada.
- Reprodução de áudio quando suportada.
- Anexos com nome, tamanho e tipo.
- Barra de progresso de upload e download.
- Cancelamento de upload quando possível.
- Cache de anexos.
- Limite de cache configurável.
- Abertura externa de arquivos.
- Copiar URL de anexos quando permitido.

## GIFs

O cliente deve oferecer suporte a GIFs.

### Requisitos

- Exibir GIFs enviados em mensagens.
- Reproduzir GIFs animados.
- Pausar animações quando a mensagem estiver fora da área visível, quando possível.
- Limitar o número de GIFs animados reproduzidos simultaneamente.
- Aplicar lazy loading.
- Evitar decodificar GIFs que não estejam visíveis.
- Permitir desativar animações nas configurações.
- Permitir reduzir animações para economizar recursos.
- Exibir preview estático antes do carregamento completo, quando disponível.
- Controlar o uso de memória durante a decodificação.
- Evitar que GIFs grandes prejudiquem a responsividade da interface.

### Não é obrigatório

- Integração com serviços comerciais de GIF.
- Catálogo proprietário de GIFs.
- Sistema de assinatura.
- Recursos pagos de GIF.
- Busca externa de GIFs, salvo se houver uma integração opcional e compatível.

---

# 4.9 Emojis e reações

## Deve existir

- Emojis Unicode.
- Emojis personalizados quando suportados.
- Reações.
- Adicionar reação.
- Remover reação.
- Lista de usuários que reagiram, quando disponível.
- Autocomplete de emojis.
- Cache de emojis personalizados.
- Suporte a emojis animados apenas se compatível com o escopo de mídia e performance.
- Configuração para reduzir animações.

---

# 4.10 Notificações

## Deve existir

- Notificações do sistema operacional.
- Notificação de DM.
- Notificação de menção.
- Notificação de resposta.
- Notificação de mensagens em canais configurados.
- Som de notificação configurável.
- Silenciar servidor.
- Silenciar canal.
- Silenciar DM.
- Horário de silêncio.
- Notificações apenas quando o aplicativo estiver em segundo plano.
- Opção de desativar previews de mensagens.
- Controle de agrupamento de notificações.
- Marcar como lido ao abrir a notificação, quando apropriado.

---

# 4.11 Configurações da interface

## Deve existir

- Tema claro.
- Tema escuro.
- Seguir tema do sistema.
- Escala da interface.
- Tamanho da fonte.
- Densidade da interface.
- Largura da lista de canais.
- Largura da lista de membros.
- Mostrar/ocultar lista de membros.
- Mostrar/ocultar avatares.
- Reduzir animações.
- Desativar GIFs animados.
- Desativar reprodução automática de mídia.
- Limitar prévias de mídia.
- Atalhos configuráveis.
- Idioma, se houver infraestrutura de tradução.
- Configuração de notificações.
- Configuração de cache.
- Configuração de privacidade local.

---

# 4.12 Privacidade e segurança local

## Deve existir

- Armazenamento seguro de credenciais.
- Não registrar tokens.
- Não enviar telemetria por padrão.
- Não utilizar publicidade.
- Não incluir rastreamento comercial.
- Configuração para limpar cache.
- Configuração para apagar dados locais.
- Exportação de logs sem dados sensíveis.
- Redação de informações privadas em logs.
- Controle de arquivos armazenados localmente.
- Opção de não armazenar histórico local além do cache necessário.
- Proteção contra carregamento ilimitado de mídia.
- Validação de arquivos recebidos.
- Limites de tamanho para cache e downloads.

---

# 5. Recursos que não devem existir

## 5.1 Recursos explicitamente fora do escopo

- Overlay de jogos.
- Transmissão de tela.
- Compartilhamento de tela.
- Streaming de jogos.
- Chamadas de vídeo.
- Câmera.
- Sistema de Nitro.
- Assinaturas.
- Loja.
- Pagamentos.
- Publicidade.
- Monetização integrada.
- Recursos cosméticos pagos.
- Perfil decorado.
- Efeitos visuais de perfil.
- Backgrounds animados.
- Recursos de marketing.
- Game launcher.
- Estatísticas de jogos.
- Integrações profundas com jogos.
- Serviços próprios obrigatórios.
- Backend próprio para substituir funcionalidades que possam funcionar diretamente com os serviços oficiais.
- Recursos que exijam infraestrutura paga para o funcionamento básico.

## 5.2 Recursos que não devem ser prioridade

- Animações decorativas.
- Transições elaboradas.
- Efeitos visuais sem função.
- Personalização excessiva de perfil.
- Sistemas de descoberta social complexos.
- Recursos de engajamento artificial.
- Funcionalidades comerciais.
- Integrações raramente utilizadas.
- Recursos que aumentem muito o consumo sem benefício claro.
- Reprodução simultânea ilimitada de mídias animadas.
- Carregamento antecipado de todo o histórico.
- Pré-carregamento de todas as imagens de todos os servidores.

---

# 6. Definição de “frufru” e critérios de decisão

Um recurso pode ser considerado “frufru” quando:

- É principalmente cosmético.
- Não melhora comunicação, navegação, moderação ou acessibilidade.
- Aumenta o consumo de CPU, RAM, rede ou disco sem benefício proporcional.
- Exige serviços externos ou infraestrutura própria.
- É voltado principalmente para monetização.
- Duplica uma função já existente.
- É raramente usado no fluxo principal.
- Complica a arquitetura sem melhorar a experiência central.

Um recurso deve ser considerado relevante quando:

- Melhora comunicação.
- Melhora navegação.
- Melhora acessibilidade.
- Melhora estabilidade.
- Melhora privacidade.
- Ajuda na administração de comunidades.
- Reduz latência percebida.
- Reduz consumo de recursos.
- É usado frequentemente.
- É necessário para uma experiência funcional de texto, DMs ou voz.

## Critérios de inclusão

Antes de adicionar qualquer funcionalidade, avaliar:

1. Qual problema ela resolve?
2. Com que frequência será usada?
3. Ela melhora comunicação ou navegação?
4. Qual o impacto em RAM?
5. Qual o impacto em CPU?
6. Qual o impacto no tempo de inicialização?
7. Exige serviço externo?
8. Aumenta a superfície de bugs?
9. Pode ser implementada de forma modular?
10. Deve ser habilitada por padrão?

---

# 7. Arquitetura proposta

```text
src/
├── main.rs
├── app/
│   ├── mod.rs
│   ├── state.rs
│   ├── actions.rs
│   └── lifecycle.rs
├── ui/
│   ├── mod.rs
│   ├── layout.rs
│   ├── servers.rs
│   ├── channels.rs
│   ├── messages.rs
│   ├── dms.rs
│   ├── members.rs
│   ├── settings.rs
│   ├── login.rs
│   ├── voice.rs
│   └── notifications.rs
├── discord/
│   ├── mod.rs
│   ├── auth.rs
│   ├── gateway.rs
│   ├── rest.rs
│   ├── models.rs
│   ├── events.rs
│   ├── rate_limits.rs
│   └── permissions.rs
├── voice/
│   ├── mod.rs
│   ├── connection.rs
│   ├── capture.rs
│   ├── playback.rs
│   ├── devices.rs
│   ├── processing.rs
│   └── push_to_talk.rs
├── storage/
│   ├── mod.rs
│   ├── config.rs
│   ├── database.rs
│   ├── cache.rs
│   ├── credentials.rs
│   └── migrations.rs
├── media/
│   ├── mod.rs
│   ├── images.rs
│   ├── gifs.rs
│   ├── audio.rs
│   ├── video.rs
│   └── downloads.rs
├── notifications/
│   ├── mod.rs
│   └── system.rs
├── shortcuts/
│   ├── mod.rs
│   └── keymap.rs
└── diagnostics/
    ├── mod.rs
    ├── logging.rs
    └── metrics.rs
```

## Camadas

```text
┌──────────────────────────────────────────┐
│                  egui UI                 │
├──────────────────────────────────────────┤
│          Application State               │
├──────────────────────────────────────────┤
│        Commands / Event Dispatcher       │
├──────────────────────────────────────────┤
│       Discord Client / Services          │
├──────────────────────┬───────────────────┤
│       Gateway        │       REST        │
│      WebSocket       │       HTTP        │
├──────────────────────┴───────────────────┤
│        Voice / Audio Subsystem            │
├──────────────────────────────────────────┤
│       Cache / Local Storage               │
├──────────────────────────────────────────┤
│      OS Integration / Notifications      │
├──────────────────────────────────────────┤
│                  Rust                    │
└──────────────────────────────────────────┘
```

---

# 8. Organização interna

## 8.1 Estado da aplicação

O estado global da aplicação deve ser separado em:

- Estado de autenticação.
- Estado de conexão.
- Estado dos servidores.
- Estado dos canais.
- Estado das mensagens.
- Estado das DMs.
- Estado das notificações.
- Estado de voz.
- Estado da interface.
- Estado de cache.
- Estado de tarefas assíncronas.
- Estado de diagnóstico.

Evitar colocar toda a lógica em um único objeto ou arquivo.

## 8.2 Comunicação entre tarefas

- Usar canais de mensagens entre tarefas assíncronas e a UI.
- Não bloquear o thread de renderização.
- Não executar downloads grandes na UI.
- Não executar decodificação pesada de GIF na UI.
- Não executar processamento de áudio pesado no thread da UI.
- Cancelar tarefas quando elas não forem mais necessárias.
- Limitar tarefas simultâneas.
- Evitar condições de corrida.
- Tratar encerramento limpo do aplicativo.

---

# 9. Fases de desenvolvimento

## Fase 0 — Pesquisa e validação técnica

- Validar os mecanismos de autenticação permitidos.
- Validar o acesso oficial às funcionalidades necessárias.
- Definir sistemas operacionais suportados.
- Testar `egui`/`eframe`.
- Testar bibliotecas de áudio.
- Testar WebSocket.
- Testar armazenamento seguro.
- Definir licença open source.
- Definir política de logs.
- Definir limites iniciais de RAM e CPU.
- Criar protótipo de benchmark.

### Entregáveis

- Documento de compatibilidade.
- Matriz de plataformas.
- Protótipo mínimo de janela.
- Protótipo de conexão.
- Decisões arquiteturais registradas.

---

## Fase 1 — Fundação

- Criar projeto Rust.
- Configurar Cargo.
- Criar módulos.
- Criar janela nativa.
- Criar loop de renderização.
- Criar sistema de estado.
- Criar sistema de comandos.
- Criar logging.
- Criar tratamento de erros.
- Criar configuração local.
- Criar keyring.
- Criar cache.
- Criar sistema básico de tarefas assíncronas.
- Criar testes unitários iniciais.
- Criar pipeline de build.

### Critérios de conclusão

- Aplicativo abre.
- Janela responde.
- Configurações são persistidas.
- Logs funcionam.
- Nenhum segredo aparece em logs.
- Build reproduzível funciona.

---

## Fase 2 — Autenticação

- Implementar tela de login.
- Implementar login por QR Code.
- Exibir QR Code.
- Controlar expiração do QR Code.
- Atualizar QR Code quando necessário.
- Processar autorização.
- Salvar sessão com segurança.
- Implementar logout.
- Implementar troca de conta.
- Implementar sessão expirada.
- Implementar limpeza de credenciais.

### Critérios de conclusão

- Fluxo de autenticação funciona pelo mecanismo autorizado.
- Sessão persiste com segurança.
- Logout remove a sessão local.
- QR Code expirado é tratado corretamente.
- Falhas são exibidas de forma clara.

---

## Fase 3 — Gateway e sincronização

- Implementar conexão WebSocket.
- Implementar handshake.
- Implementar identificação.
- Implementar heartbeat.
- Implementar reconexão.
- Implementar retomada de sessão quando suportada.
- Implementar fila de eventos.
- Implementar atualização de servidores.
- Implementar atualização de canais.
- Implementar atualização de membros.
- Implementar atualização de mensagens.
- Implementar atualização de status.
- Implementar eventos de voz.
- Implementar indicador de conexão.

### Critérios de conclusão

- O cliente conecta e reconecta.
- Eventos são processados corretamente.
- A UI não trava durante reconexões.
- O estado local não fica inconsistente após uma queda.

---

## Fase 4 — API e mensagens

- Implementar cliente HTTP.
- Implementar histórico.
- Implementar envio.
- Implementar edição.
- Implementar exclusão.
- Implementar replies.
- Implementar reações.
- Implementar mensagens fixadas.
- Implementar busca.
- Implementar anexos.
- Implementar downloads.
- Implementar rate limits.
- Implementar tratamento de erros.
- Implementar paginação.
- Implementar cache de dados.

### Critérios de conclusão

- Mensagens são carregadas.
- Mensagens podem ser enviadas.
- Edição e exclusão funcionam quando permitidas.
- O histórico é carregado incrementalmente.
- Rate limits não causam loops de requisições.
- Falhas de rede são tratadas.

---

## Fase 5 — Interface principal

- Barra de servidores.
- Lista de canais.
- Área de mensagens.
- Campo de texto.
- Lista de membros.
- DMs.
- Threads.
- Replies.
- Markdown.
- Menções.
- Emojis.
- Reações.
- Mensagens não lidas.
- Busca.
- Quick Switcher.
- Atalhos.
- Temas.
- Configurações.
- Notificações.

### Critérios de conclusão

- Navegação principal funciona.
- Mensagens são legíveis.
- O cliente permanece responsivo em canais movimentados.
- A UI suporta teclado.
- Mensagens não lidas são preservadas corretamente.

---

## Fase 6 — Mídia e GIFs

- Upload de arquivos.
- Download de anexos.
- Preview de imagens.
- Reprodução de áudio.
- Visualização de vídeos quando suportada.
- Suporte a GIFs animados.
- Decodificação sob demanda.
- Pausa fora da área visível.
- Limite de GIFs simultâneos.
- Redução de animações.
- Configuração para desativar GIFs.
- Cache de mídia.
- Limites de tamanho.
- Cancelamento de downloads.

### Critérios de conclusão

- GIFs são exibidos corretamente.
- GIFs não bloqueiam a UI.
- GIFs fora da área visível não consomem recursos desnecessariamente.
- O consumo de memória é limitado.
- O usuário pode desativar animações.

---

## Fase 7 — Voz e áudio

- Detectar dispositivos.
- Selecionar entrada.
- Selecionar saída.
- Capturar microfone.
- Reproduzir áudio.
- Entrar em canal de voz.
- Sair de canal de voz.
- Reconectar.
- Push-to-talk.
- Sensibilidade.
- Teste de microfone.
- Teste de saída.
- Volume individual.
- Supressão de ruído.
- Cancelamento de eco.
- Controle automático de ganho.
- Indicadores de atividade.
- Configurações de processamento.

### Critérios de conclusão

- Usuário entra e sai de canais de voz.
- Entrada e saída podem ser selecionadas.
- Push-to-talk funciona.
- Sensibilidade funciona.
- Testes de áudio funcionam.
- Falhas de dispositivos são tratadas.
- Processamento de voz pode ser ativado e desativado.

---

## Fase 8 — Performance e estabilidade

- Medir tempo de inicialização.
- Medir RAM em repouso.
- Medir RAM com histórico grande.
- Medir RAM com GIFs.
- Medir CPU em repouso.
- Medir CPU durante navegação.
- Medir CPU durante voz.
- Medir tempo de renderização.
- Medir latência percebida.
- Reduzir alocações.
- Otimizar cache.
- Otimizar renderização.
- Otimizar carregamento de imagens.
- Otimizar GIFs.
- Otimizar listas grandes.
- Implementar virtualização de mensagens quando necessário.
- Reduzir tarefas em segundo plano.
- Corrigir vazamentos.
- Testar reconexão.
- Testar suspensão e retomada.

### Critérios de conclusão

- Metas de performance são medidas em hardware definido.
- Não existem travamentos recorrentes.
- O uso de RAM permanece controlado.
- O cliente continua utilizável com muitos servidores e mensagens.
- O modo ocioso consome poucos recursos.

---

## Fase 9 — Qualidade e distribuição

- Testes unitários.
- Testes de integração.
- Testes de interface.
- Testes de rede.
- Testes de reconexão.
- Testes de áudio.
- Testes de mídia.
- Testes de GIF.
- Testes de segurança local.
- Testes de migração de configuração.
- Testes em Windows.
- Testes em Linux.
- Testes em macOS, se suportado.
- Empacotamento.
- Assinatura de binários quando possível.
- Documentação de instalação.
- Documentação de configuração.
- Changelog.
- Política de contribuição.
- Licença.
- Relatórios de benchmark.

---

# 10. Testes obrigatórios

## Rede

- Conexão inicial.
- Queda de conexão.
- Reconexão.
- Timeout.
- Rate limit.
- Resposta inválida.
- Sessão expirada.
- Gateway indisponível.
- Eventos fora de ordem.
- Duplicação de eventos.

## Mensagens

- Mensagem curta.
- Mensagem longa.
- Markdown.
- Código.
- Menção.
- Reply.
- Edição.
- Exclusão.
- Reação.
- Anexo.
- Mensagem com embed.
- Mensagens não lidas.
- Histórico grande.

## Mídia

- Imagem pequena.
- Imagem grande.
- GIF pequeno.
- GIF grande.
- Muitos GIFs simultâneos.
- GIF fora da área visível.
- GIF corrompido.
- Arquivo inválido.
- Download interrompido.
- Cache cheio.

## Voz

- Microfone padrão.
- Microfone alternativo.
- Saída padrão.
- Saída alternativa.
- Troca de dispositivo.
- Push-to-talk.
- Sensibilidade.
- Supressão de ruído.
- Cancelamento de eco.
- Queda de conexão.
- Entrada sem sinal.
- Dispositivo removido.

## Performance

- Inicialização limpa.
- Inicialização com cache.
- Muitos servidores.
- Muitos canais.
- Histórico extenso.
- Muitos anexos.
- Muitos GIFs.
- Canal de voz ativo.
- Aplicativo minimizado.
- Aplicativo ocioso.

---

# 11. Métricas e benchmarks

Registrar pelo menos:

- Tempo até a janela aparecer.
- Tempo até a UI estar utilizável.
- Tempo até a primeira mensagem aparecer.
- RAM após inicialização.
- RAM em repouso.
- RAM com vários servidores.
- RAM com histórico extenso.
- RAM com GIFs.
- CPU em repouso.
- CPU durante rolagem.
- CPU durante voz.
- Tempo médio de renderização.
- Picos de CPU.
- Picos de RAM.
- Tempo de reconexão.
- Tempo de carregamento de mídia.
- Quantidade de tarefas ativas.
- Tamanho do cache.
- Tamanho do binário.

Os benchmarks devem ser executados em hardware, sistema operacional e configuração documentados.

---

# 12. Compatibilidade de plataformas

## Windows

- Windows Credential Manager.
- Notificações nativas.
- Dispositivos de áudio do Windows.
- Empacotamento nativo.
- Atalhos e integração com bandeja, se desejado.

## Linux

- Secret Service/libsecret.
- Notificações compatíveis com o ambiente.
- PipeWire/ALSA/PulseAudio conforme a implementação escolhida.
- Empacotamento para distribuições definidas.

## macOS

- Keychain.
- Notificações nativas.
- Dispositivos de áudio do macOS.
- Empacotamento e assinatura quando possível.

A lista final de plataformas deve ser definida na Fase 0.

---

# 13. Configuração inicial sugerida

## Geral

- Iniciar com o sistema.
- Minimizar para bandeja, se implementado.
- Tema.
- Escala.
- Idioma.
- Atalhos.

## Aparência

- Densidade da interface.
- Tamanho da fonte.
- Mostrar avatares.
- Mostrar lista de membros.
- Reduzir animações.
- Desativar GIFs animados.
- Desativar reprodução automática.

## Notificações

- Ativar notificações.
- Ativar sons.
- Mostrar preview.
- Silenciar durante horário definido.
- Configurar notificações por servidor e canal.

## Voz e vídeo

- Dispositivo de entrada.
- Dispositivo de saída.
- Volume de entrada.
- Volume de saída.
- Sensibilidade.
- Push-to-talk.
- Supressão de ruído.
- Cancelamento de eco.
- Controle automático de ganho.

## Dados

- Limite de cache.
- Pasta de cache.
- Limpar cache.
- Apagar dados locais.
- Não armazenar histórico além do necessário.
- Exportar logs sem dados sensíveis.

---

# 14. Critérios de sucesso

O projeto será considerado funcional quando:

- O aplicativo abrir rapidamente.
- A interface permanecer responsiva.
- O usuário conseguir autenticar-se pelo mecanismo implementado.
- Servidores e canais forem carregados.
- Mensagens puderem ser lidas, enviadas, editadas e excluídas quando permitido.
- DMs funcionarem.
- Reações e replies funcionarem.
- Busca funcionar.
- Arquivos e imagens funcionarem.
- GIFs forem exibidos sem comprometer a estabilidade.
- Notificações funcionarem.
- O usuário conseguir entrar em canais de voz.
- O usuário conseguir escolher entrada e saída de áudio.
- Sensibilidade e Push-to-talk funcionarem.
- Processamento de voz disponível puder ser configurado.
- Credenciais não forem salvas em texto puro.
- O cliente não depender de Electron ou Chromium.
- O consumo de recursos for medido e documentado.
- O código puder ser compilado e executado a partir de uma instalação limpa.

---

# 15. Resultado esperado

O resultado deve ser um cliente Discord nativo, rápido e enxuto, com foco em:

- Texto.
- DMs.
- Servidores.
- Canais.
- Busca.
- Reações.
- Threads.
- Arquivos.
- Imagens.
- GIFs.
- Notificações.
- Voz.
- Configuração completa de entrada e saída de áudio.
- Sensibilidade do microfone.
- Push-to-talk.
- Supressão de ruído quando possível.
- Baixo consumo de recursos.
- Privacidade local.
- Código aberto.

O projeto não deve tentar reproduzir recursos comerciais, cosméticos ou periféricos do Discord. A prioridade é entregar uma experiência funcional, estável, rápida e nativa para comunicação diária.
