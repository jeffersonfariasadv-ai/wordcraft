//! AutoSave without a manual Save (fork customisation, kept in one file so upstream merges stay
//! simple).
//!
//! - A file opened in a format AutoSave keeps (.docx, .odt, .rtf…) is AutoSaved from the first
//!   edit. Before WordCraft first writes over it in this session (AutoSave or Save), the original
//!   is copied beside it as `NAME.ext.bak`. If that copy can't be made, AutoSave stays off for
//!   the file and says why.
//! - A file in a format WordCraft can't keep (.doc, .md, .txt…) or one with a password is not
//!   AutoSaved, and opening it says why on the status bar.
//! - A new document is asked once, after its first edit, where to save (Save As). Cancelling
//!   leaves it with its recovery copy only, and it isn't asked again this session. With
//!   File › Options › Save › default folder set, it is saved there as
//!   `Documento AAAA-MM-DD HHhMM.docx` without asking.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::{AutoSaveBlock, Value, WordApp, i18n, json, keeps_everything};

/// What this customisation remembers for the session.
#[derive(Default)]
pub(crate) struct ForkAutoSave {
    /// Files opened in this session whose original hasn't been copied to `.bak` yet.
    needs_backup: HashSet<PathBuf>,
    /// New documents (by id) already asked where to save, so they're never asked twice.
    asked: HashSet<u64>,
}

/// The backup kept beside `path`: its whole file name with `.bak` added (`Petição.docx.bak`).
pub fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(std::ffi::OsStr::to_os_string).unwrap_or_default();
    name.push(".bak");
    path.with_file_name(name)
}

/// Copy `from` to `to`, which Windows keeps hidden (the folder shows it only with "Hidden items"
/// on), so the backup doesn't clutter the user's folders. Elsewhere a plain copy.
pub fn copy_hidden(from: &Path, to: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        let mut src = std::fs::File::open(from)?;
        let mut dst = std::fs::OpenOptions::new().write(true).create(true).truncate(true).attributes(FILE_ATTRIBUTE_HIDDEN).open(to)?;
        std::io::copy(&mut src, &mut dst)?;
        dst.sync_all()
    }
    #[cfg(not(windows))]
    {
        std::fs::copy(from, to).map(|_| ())
    }
}

/// The name a new document gets in the default folder: `Documento 2026-11-07 14h05.docx`.
pub fn new_document_name(now: chrono::NaiveDateTime) -> String {
    format!("Documento {}.docx", now.format("%Y-%m-%d %Hh%M"))
}

/// `name` in `dir`, or `name (2)`, `name (3)`… when a file already has it. `None` after 999.
pub fn unused_path(dir: &Path, name: &str) -> Option<PathBuf> {
    let first = dir.join(name);
    if !first.exists() {
        return Some(first);
    }
    let p = Path::new(name);
    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = p.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
    (2..1000).map(|n| dir.join(format!("{stem} ({n}).{ext}"))).find(|c| !c.exists())
}

impl WordApp {
    /// After `file.open` succeeded (`result` is its answer): let AutoSave cover the opened file
    /// when it can, or say why it can't.
    pub(crate) fn fork_after_open(&mut self, result: &Value) {
        if result.get("alreadyOpen").and_then(Value::as_bool) == Some(true) {
            return;
        }
        // Only a file that was actually opened (not a picker that was just shown).
        let opened = result.get("path").and_then(Value::as_str).map(Path::new);
        if opened.is_none() || opened != self.session.path.as_deref() {
            return;
        }
        // The browser can't write to the user's files.
        if self.services.download.is_some() || self.services.host_save.is_some() {
            return;
        }
        let Some(path) = self.session.path.clone() else { return };
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if self.session.password.is_some() {
            if self.autosave {
                self.status(i18n::fmt(
                    tl!("{name} tem senha: o salvamento automático fica desligado para ele. Salve com Ctrl+S."),
                    &[("name", &name)],
                ));
            }
            return;
        }
        if !keeps_everything(&path.to_string_lossy()) {
            if self.autosave {
                self.status(i18n::fmt(
                    tl!("Salvamento automático desligado: o WordCraft não grava {name} nesse formato sem perder formatação. Clique no botão de salvamento automático (ou Arquivo › Salvar como) e escolha .docx para ligá-lo."),
                    &[("name", &name)],
                ));
            }
            return;
        }
        self.autosave_path = Some(path.clone());
        self.fork.needs_backup.insert(path);
    }

    /// After `file.rename` moved the document's file from `old`: a backup still owed to the
    /// original is owed under its new name.
    pub(crate) fn fork_after_rename(&mut self, old: &Path) {
        if self.fork.needs_backup.remove(old)
            && let Some(new) = self.session.path.clone()
        {
            self.fork.needs_backup.insert(new);
        }
    }

    /// Before `file.save` / `file.saveAs` runs with `params`: if it writes over a file opened in
    /// this session that has no backup yet, copy the original to `.bak` first. A failure is
    /// reported and the save goes on (the user asked for it).
    pub(crate) fn fork_backup_before_save(&mut self, id: &str, params: &Value) {
        if !matches!(id, "file.save" | "file.saveAs") {
            return;
        }
        let target = match params.get("path").and_then(Value::as_str) {
            Some(p) => PathBuf::from(p),
            None if id == "file.save" => match &self.session.path {
                Some(p) => p.clone(),
                None => return,
            },
            None => return,
        };
        if let Err(e) = self.fork_backup(&target) {
            self.status(e);
        }
        // The user chose to write over it: from now on the file is theirs, not the original.
        self.fork.needs_backup.remove(&target);
    }

    /// Copy `path` to its `.bak` if it is a file opened in this session that has none yet.
    fn fork_backup(&mut self, path: &Path) -> Result<(), String> {
        if !self.fork.needs_backup.contains(path) {
            return Ok(());
        }
        if path.is_file() {
            let bak = backup_path(path);
            let bak_s = bak.to_string_lossy().into_owned();
            wordcraft_engine::file_access::check_write(&bak_s)?;
            copy_hidden(path, &bak).map_err(|e| {
                log::warn!("couldn't back up {} to {bak_s}: {e}", path.display());
                i18n::fmt(tl!("Não foi possível criar a cópia de segurança {bak}: {error}"), &[("bak", &bak_s), ("error", &e.to_string())])
            })?;
            log::info!("backed up {} to {bak_s}", path.display());
        }
        self.fork.needs_backup.remove(path);
        Ok(())
    }

    /// Before AutoSave writes: the original's backup ([`Self::fork_backup`]). On failure AutoSave
    /// is turned off for the file, so it is never overwritten without its backup.
    pub(crate) fn fork_backup_before_autosave(&mut self) -> Result<(), String> {
        let Some(path) = self.autosave_path.clone() else { return Ok(()) };
        self.fork_backup(&path).inspect_err(|_| self.autosave_path = None)
    }

    /// Closing (or quitting, or anything else that would discard it) a document AutoSave covers:
    /// save its last changes now, after the original's backup, so nothing asks Save / Don't Save
    /// for edits AutoSave hadn't written yet. A failure is shown and the usual question follows.
    pub(crate) fn fork_save_before_discard(&mut self) {
        if !self.autosaves() || !self.session.dirty {
            return;
        }
        let saved = self.fork_backup_before_autosave().and_then(|()| self.session.run("file.save", &json!({})).map_err(|e| e.to_string()));
        if let Err(e) = saved {
            log::warn!("AutoSave before closing failed: {e}");
            self.status(i18n::fmt(tl!("Não foi possível salvar automaticamente ao fechar: {error}"), &[("error", &e)]));
        }
    }

    /// A new document with changes, once the user pauses: ask once where to save it (or save it
    /// in the default folder), so AutoSave can cover it from then on.
    pub(crate) fn fork_ask_where_to_save(&mut self, now: f64) {
        let id = self.session.document_id();
        let ready = self.autosave
            && self.session.dirty
            && self.has_document()
            && !self.ui.backstage
            && self.dialog.is_none()
            && !self.file_dialog_open()
            && now - self.last_autosave > 2500.0
            && now - self.canvas.caret_visible_since > 1500.0
            && self.autosave_block() == Some(AutoSaveBlock::Unsaved)
            && !self.fork.asked.contains(&id);
        if !ready {
            return;
        }
        self.fork.asked.insert(id);
        self.last_autosave = now;
        if let Some(path) = self.fork_default_save_path() {
            let shown = path.to_string_lossy().into_owned();
            match self.run("file.saveAs", json!({"path": shown})) {
                Ok(v) if v.get("saved").and_then(Value::as_bool) == Some(true) => {
                    self.status(i18n::fmt(tl!("Documento salvo automaticamente em {path}"), &[("path", &shown)]));
                    return;
                }
                Ok(_) => {}
                Err(e) => {
                    log::warn!("saving the new document in the default folder failed: {e}");
                    self.status(i18n::fmt(tl!("Não foi possível salvar na pasta padrão: {error}"), &[("error", &e)]));
                }
            }
        }
        let _ = self.save_as(crate::file_dialogs::AfterSave::AutoSave);
    }

    /// Where a new document is saved without asking: an unused `Documento … .docx` name in
    /// File › Options' default folder, when one is set and exists.
    fn fork_default_save_path(&self) -> Option<PathBuf> {
        let folder = self.session.prefs.new_documents_folder.as_deref().map(str::trim).filter(|f| !f.is_empty())?;
        let dir = Path::new(folder);
        if !dir.is_dir() {
            log::debug!("default folder for new documents {folder:?} isn't a folder");
            return None;
        }
        #[cfg(not(target_arch = "wasm32"))]
        let name = new_document_name(chrono::Local::now().naive_local());
        #[cfg(target_arch = "wasm32")]
        let name = String::from("Documento.docx");
        unused_path(dir, &name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_keeps_the_whole_name() {
        assert_eq!(backup_path(Path::new("/x/Petição.docx")), PathBuf::from("/x/Petição.docx.bak"));
        assert_eq!(backup_path(Path::new("notas.odt")), PathBuf::from("notas.odt.bak"));
    }

    /// Windows: the backup is a hidden file, and a second backup over it still works.
    #[cfg(windows)]
    #[test]
    fn backups_are_hidden_on_windows() {
        use std::os::windows::fs::MetadataExt;
        let dir = std::env::temp_dir().join(format!("wc-fork-hidden-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("a.docx");
        std::fs::write(&src, b"one").unwrap();
        let bak = backup_path(&src);
        copy_hidden(&src, &bak).unwrap();
        assert_eq!(std::fs::metadata(&bak).unwrap().file_attributes() & 0x2, 0x2, "hidden");
        std::fs::write(&src, b"two").unwrap();
        copy_hidden(&src, &bak).unwrap();
        assert_eq!(std::fs::read(&bak).unwrap(), b"two");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn new_document_names_follow_the_pattern() {
        let t = chrono::NaiveDate::from_ymd_opt(2026, 11, 7).and_then(|d| d.and_hms_opt(9, 5, 0)).unwrap();
        assert_eq!(new_document_name(t), "Documento 2026-11-07 09h05.docx");
    }

    #[test]
    fn unused_path_numbers_taken_names() {
        let dir = std::env::temp_dir().join(format!("wc-fork-unused-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(unused_path(&dir, "Doc.docx"), Some(dir.join("Doc.docx")));
        std::fs::write(dir.join("Doc.docx"), b"x").unwrap();
        assert_eq!(unused_path(&dir, "Doc.docx"), Some(dir.join("Doc (2).docx")));
        std::fs::write(dir.join("Doc (2).docx"), b"x").unwrap();
        assert_eq!(unused_path(&dir, "Doc.docx"), Some(dir.join("Doc (3).docx")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
