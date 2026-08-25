use crate::{
    cli::Repo,
    error::{Error, Result},
    plan::{RemoteFile, format_bytes},
};
use inquire::MultiSelect;

struct FileItem(RemoteFile);

impl std::fmt::Display for FileItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.0.path, format_bytes(self.0.size))
    }
}

pub fn prompt_selection(repo: &Repo, files: Vec<RemoteFile>) -> Result<Option<Vec<RemoteFile>>> {
    if files.is_empty() {
        return Ok(Some(files));
    }

    let items: Vec<FileItem> = files.into_iter().map(FileItem).collect();
    let prompt_msg = format!("Select files to download from {}:", repo.full());
    let page_size = 15.min(items.len().max(5));

    let ans = MultiSelect::new(&prompt_msg, items)
        .with_page_size(page_size)
        .with_help_message(
            "↑↓ move, space select/deselect, 'a' toggle all, type to filter, enter confirm",
        )
        .prompt();

    match ans {
        Ok(selected) => Ok(Some(selected.into_iter().map(|item| item.0).collect())),
        Err(
            inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted,
        ) => Ok(None),
        Err(err) => Err(Error::Prompt(err.to_string())),
    }
}
