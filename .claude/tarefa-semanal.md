Você vai atualizar a versão personalizada do WordCraft do Jefferson com as novidades do projeto original. O Jefferson é leigo em programação: o relatório final deve ser curto, em português simples, sem jargão.

## Contexto
- Projeto local: `E:\SISTEMAS VS CODE\wordcraft.my` (git). `origin` = https://github.com/jeffersonfariasadv-ai/wordcraft (fork dele). `upstream` = https://github.com/storytold/wordcraft (original). Se o remote `upstream` não existir, crie-o com essa URL.
- O branch `main` do fork é a versão que ele usa. O programa que ele abre é `E:\SISTEMAS VS CODE\wordcraft.my\target\release\wordcraft.exe`.
- As personalizações dele (preservar SEMPRE em conflitos): o arquivo `crates/ui-egui/src/autosave_fork.rs` e todo trecho marcado com comentário "Fork:" no código (salvamento automático sem clicar em Salvar, cópia .bak oculta, pasta padrão para novos documentos em `crates/engine/src/prefs.rs` e `crates/engine/src/cmd/options.rs`, aviso de atualização desligado em `apps/wordcraft/src/main.rs` com `set_startup_update_check(false)`), além de `.claude/commands/atualizar.md` (adicionado com `git add -f`, pois `.claude/` é ignorado).
- Traduções: pasta `E:\SISTEMAS VS CODE\craft-i18n` (clone de https://github.com/storytold/craft-i18n). Ela deve estar no commit indicado no arquivo `craft-i18n.rev` do projeto.
- Em todo comando cargo, no shell Bash: `export PATH="$HOME/.cargo/bin:$PATH"; export CRAFT_I18N_DIR="E:/SISTEMAS VS CODE/craft-i18n"; export CARGO_BUILD_JOBS=8`. Compilações longas: rode em segundo plano e espere terminar.
- Regras do projeto: leia AGENTS.md antes de resolver conflitos (sem unwrap/expect/panic em código que não seja teste).

## Passos
1. `git status`: se houver alterações não commitadas, NÃO mexa em nada; relate isso e pare.
2. Memória: se a memória virtual livre do Windows for menor que 8 GB (PowerShell: `(Get-CimInstance Win32_OperatingSystem).FreeVirtualMemory/1MB`), não compile: relate que faltou memória (provavelmente um jogo ou programa pesado aberto) e pare. Se a compilação falhar com "only metadata stub found for `rlib` dependency `core`", é falta de memória: tente de novo com `CARGO_BUILD_JOBS=2`.
3. `git checkout main`, `git pull origin main`, `git fetch upstream`. Se `git rev-list --count main..upstream/main` for 0, não há novidades: relate "Nenhuma novidade nesta semana" e pare.
4. Crie o branch `atualizacao-AAAA-MM-DD` (data de hoje) a partir do `main` e rode `git merge --no-edit upstream/main`. Resolva conflitos preservando as personalizações acima e incorporando as mudanças do original; anote o que decidiu.
5. Atualize a pasta craft-i18n para o commit de `craft-i18n.rev`.
6. Verifique: `cargo fmt --all -- --check`, `cargo clippy -p wordcraft -p wordcraft-ui-egui -p wordcraft-engine --all-targets -- -D warnings`, `cargo test -p wordcraft-ui-egui`, `cargo test -p wordcraft-engine`, `cargo test -p wordcraft-doc -p wordcraft-docx`.
7. Se TUDO passar: `git checkout main`, `git merge --ff-only atualizacao-AAAA-MM-DD`, `git push origin main`, apague o branch local. Depois recompile o programa: se o processo `wordcraft` estiver aberto (`tasklist | findstr /i wordcraft`), não feche o programa (ele pode ter trabalho não salvo); apenas relate que a atualização está pronta e que basta fechar o WordCraft e pedir ao Claude "recompile o WordCraft" (`cargo build --release -p wordcraft`). Se estiver fechado, rode `cargo build --release -p wordcraft`.
8. Se algo falhar (conflito que você não consegue resolver com segurança, teste ou compilação): NÃO altere o `main` e NÃO faça push. Deixe o branch local `atualizacao-AAAA-MM-DD` para análise, volte para o `main` (`git merge --abort` se necessário) e relate o problema em linguagem simples, com a sugestão de solução.

## Proibido
Nunca faça push para storytold/wordcraft, nunca use `--force`, nunca altere o branch de outra forma que não as acima, nunca baixe ou instale o WordCraft oficial.

## Relatório final (curto, em português simples)
- Se atualizou: quantas novidades entraram, um resumo de 3 a 5 itens do que mudou para o usuário (leia as mensagens dos commits novos), se houve conflito e como foi resolvido, e se o programa já foi recompilado ou se ele precisa fechar o WordCraft e pedir a recompilação.
- Se não atualizou: o motivo e o que fazer.
