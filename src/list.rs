use crate::models::{Declaration, DeclarationStatus};
use crate::storage::Storage;

pub struct ListOptions {
    pub show_all: bool,
    pub status: Option<DeclarationStatus>,
}

#[must_use]
pub fn list_declarations(storage: &Storage, options: &ListOptions) -> Vec<Declaration> {
    let status_filter = if options.show_all {
        None
    } else {
        options.status.as_ref()
    };

    let mut decls = storage.get_declarations(status_filter, None);

    if !options.show_all && options.status.is_none() {
        decls.retain(|d| d.status != DeclarationStatus::Finalized);
    }

    decls.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    decls
}
