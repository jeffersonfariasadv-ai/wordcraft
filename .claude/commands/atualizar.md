---
description: Atualiza o fork com as melhorias do WordCraft original
---
Atualize este fork com o projeto original (storytold/wordcraft). Explique cada passo em linguagem simples: o usuário é leigo.

1. Confirme que não há alterações pendentes (`git status`). Se houver, pare e me pergunte.
2. Se o remote `upstream` não existir, crie com: `git remote add upstream https://github.com/storytold/wordcraft.git`
3. `git checkout main` e `git pull origin main`, depois `git fetch upstream`.
4. Crie o branch `atualizacao-AAAA-MM-DD` (data de hoje) a partir do `main` e rode `git merge upstream/main`.
5. Se houver conflito, resolva preservando as minhas personalizações (salvamento automático em `crates/ui-egui/src/autosave_fork.rs` e os pontos marcados com "Fork:" no código; marca d'água com imagem, quando existir) e me explique em linguagem simples o que decidiu.
6. Compile e teste, com as traduções em português (`CRAFT_I18N_DIR` apontando para a pasta `craft-i18n` ao lado do projeto, no commit indicado em `craft-i18n.rev`):
   - `cargo build --release -p wordcraft`
   - `cargo test -p wordcraft-ui-egui autosave`
   - `cargo test -p wordcraft-doc -p wordcraft-engine -p wordcraft-docx`
7. Se tudo passar, faça push do branch para o `origin` (meu fork) e abra um pull request para o MEU `main` (nunca para o storytold) com um resumo das novidades. Não faça merge sozinho.
8. Se algo falhar, não faça push: me explique o problema e sugira a solução.
