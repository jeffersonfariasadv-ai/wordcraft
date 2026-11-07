# Minha versão do WordCraft (fork de jeffersonfariasadv-ai)

Esta é uma versão personalizada do [WordCraft](https://github.com/storytold/wordcraft). Todas as
alterações estão neste repositório; nada precisa ser refeito.

## Personalizações

- **Salvamento automático sem clicar em Salvar:** arquivos abertos (.docx, .odt, .rtf) são salvos
  sozinhos; documentos novos perguntam uma vez onde salvar (ou usam a pasta padrão de Arquivo ›
  Opções › Salvar); ao fechar, salva sem perguntar. Antes da primeira gravação, o original é
  copiado como `NOME.docx.bak` (arquivo oculto).
- **Sem aviso de atualização** do WordCraft oficial: as novidades chegam pelo comando `/atualizar`
  e pela tarefa semanal (instruções em `.claude/tarefa-semanal.md`).
- Código: `crates/ui-egui/src/autosave_fork.rs` e os trechos marcados com "Fork:".

## Restaurar num computador novo

Abra o Claude Code numa pasta vazia (por exemplo `E:\SISTEMAS VS CODE\wordcraft.my`) e peça:

> Restaure minha versão do WordCraft seguindo o FORK.md de
> https://github.com/jeffersonfariasadv-ai/wordcraft e me explique cada passo, sou leigo.

Passos que o Claude Code deve seguir:

1. Instalar o que faltar: Git, GitHub CLI (`gh`, e fazer login), Visual Studio 2022 Build Tools
   com "Desenvolvimento para desktop com C++" e Rust (`rustup`).
2. Clonar o fork: `git clone https://github.com/jeffersonfariasadv-ai/wordcraft.git .` e criar o
   upstream: `git remote add upstream https://github.com/storytold/wordcraft.git`.
3. Por privacidade (o repositório é público), usar o e-mail anônimo do GitHub neste projeto:
   `git config user.email "251276013+jeffersonfariasadv-ai@users.noreply.github.com"`.
4. Traduções em português: clonar https://github.com/storytold/craft-i18n na pasta ao lado
   (`..\craft-i18n`) e fazer checkout do commit escrito em `craft-i18n.rev`.
5. Compilar (no Bash): `export CRAFT_I18N_DIR="<pasta do craft-i18n>"; export CARGO_BUILD_JOBS=8;
   cargo build --release -p wordcraft`. Se aparecer o erro "only metadata stub found for `rlib`
   dependency `core`", falta memória: feche programas pesados ou use `CARGO_BUILD_JOBS=2`.
6. O programa fica em `target\release\wordcraft.exe`. Criar um atalho na Área de Trabalho e, se
   eu quiser, associar os arquivos .docx a ele.
7. Recriar a tarefa agendada semanal (domingo, 23h) com o conteúdo de
   `.claude/tarefa-semanal.md`, na pasta do projeto.
