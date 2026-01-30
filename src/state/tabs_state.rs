//! TabsState - Tab Navigation State

use crate::app::navigation::{ActivePage, Tab};

/// State for tab navigation
#[derive(Debug)]
pub struct TabsState {
    /// Currently active page
    pub active_page: ActivePage,
    /// Open tabs
    pub tabs: Vec<Tab>,
    /// Next tab ID
    next_id: u64,
}

impl Default for TabsState {
    fn default() -> Self {
        let home_tab = Tab::new(1, ActivePage::Home);
        Self {
            active_page: ActivePage::Home,
            tabs: vec![home_tab],
            next_id: 2,
        }
    }
}

impl TabsState {
    /// Set the active page (from sidebar click)
    pub fn set_active_page(&mut self, page: ActivePage) {
        self.active_page = page;

        // Create tab if it doesn't exist
        if !self.tabs.iter().any(|t| t.page == page) {
            let tab = Tab::new(self.next_id, page);
            self.next_id += 1;
            self.tabs.push(tab);
        }
    }

    /// Close a tab by ID
    pub fn close_tab(&mut self, tab_id: u64) {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == tab_id) {
            let tab = &self.tabs[pos];

            // Don't close home tab
            if !tab.closable {
                return;
            }

            // If closing the active tab, switch to another
            if tab.page == self.active_page {
                // Try to switch to the tab before or after
                let new_active = if pos > 0 {
                    self.tabs[pos - 1].page
                } else if pos + 1 < self.tabs.len() {
                    self.tabs[pos + 1].page
                } else {
                    ActivePage::Home
                };
                self.active_page = new_active;
            }

            self.tabs.remove(pos);
        }
    }

    /// Get the active tab
    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.page == self.active_page)
    }
}
