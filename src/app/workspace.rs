//! Workspace - Main Shell with Layout and Event Pump
//!
//! The workspace is the main container that holds the header, sidebar, content area, and log panel.
//! It also manages the event pump that bridges service events to UI updates.

use gpui::{
    div, prelude::*, App, Context, Entity, IntoElement, ParentElement, Render,
    Styled, Window,
};

use crate::app::entities::AppEntities;
use crate::app::navigation::ActivePage;
use crate::components::layout::header::Header;
use crate::components::layout::log_panel::LogPanel;
use crate::components::layout::sidebar::Sidebar;
use crate::eventing::app_event::AppEvent;
use crate::features::commands::page::CommandsPage;
use crate::features::events::page::EventsPage;
use crate::features::home::page::HomePage;
use crate::features::properties::page::PropertiesPage;
use crate::theme::colors::DfcColors;

/// Main workspace containing the application layout
pub struct Workspace {
    entities: AppEntities,
    header: Entity<Header>,
    sidebar: Entity<Sidebar>,
    log_panel: Entity<LogPanel>,
    // Page views (created lazily or cached)
    home_page: Option<Entity<HomePage>>,
    properties_page: Option<Entity<PropertiesPage>>,
    events_page: Option<Entity<EventsPage>>,
    commands_page: Option<Entity<CommandsPage>>,
}

impl Workspace {
    pub fn new(
        entities: AppEntities,
        event_rx: flume::Receiver<AppEvent>,
        cx: &mut Context<Self>,
    ) -> Self {
        // Create layout components
        let header = cx.new(|cx| Header::new(entities.clone(), cx));
        let sidebar = cx.new(|cx| Sidebar::new(entities.clone(), cx));
        let log_panel = cx.new(|cx| LogPanel::new(entities.clone(), cx));

        // Create home page (always visible initially)
        let home_page = Some(cx.new(|cx| HomePage::new(entities.clone(), cx)));

        // Start event pump
        Self::start_event_pump(event_rx, entities.clone(), cx);

        // Observe tabs state for page changes
        cx.observe(&entities.tabs, |this, _, cx| {
            cx.notify();
        })
        .detach();

        Self {
            entities,
            header,
            sidebar,
            log_panel,
            home_page,
            properties_page: None,
            events_page: None,
            commands_page: None,
        }
    }

    /// Start the event pump that dispatches service events to UI
    fn start_event_pump(
        event_rx: flume::Receiver<AppEvent>,
        entities: AppEntities,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |_this, cx| {
            while let Ok(event) = event_rx.recv_async().await {
                let entities = entities.clone();
                let _ = cx.update(|cx: &mut App| {
                    dispatch_event(event, &entities, cx);
                });
            }
        })
        .detach();
    }

    /// Get or create a page view for the given page
    fn get_or_create_page(&mut self, page: ActivePage, cx: &mut Context<Self>) -> impl IntoElement {
        match page {
            ActivePage::Home => {
                if self.home_page.is_none() {
                    self.home_page = Some(cx.new(|cx| HomePage::new(self.entities.clone(), cx)));
                }
                self.home_page.clone().unwrap().into_any_element()
            }
            ActivePage::Properties => {
                if self.properties_page.is_none() {
                    self.properties_page =
                        Some(cx.new(|cx| PropertiesPage::new(self.entities.clone(), cx)));
                }
                self.properties_page.clone().unwrap().into_any_element()
            }
            ActivePage::Events => {
                if self.events_page.is_none() {
                    self.events_page =
                        Some(cx.new(|cx| EventsPage::new(self.entities.clone(), cx)));
                }
                self.events_page.clone().unwrap().into_any_element()
            }
            ActivePage::Commands => {
                if self.commands_page.is_none() {
                    self.commands_page =
                        Some(cx.new(|cx| CommandsPage::new(self.entities.clone(), cx)));
                }
                self.commands_page.clone().unwrap().into_any_element()
            }
            // TODO: Implement other pages
            _ => div()
                .p_4()
                .child(format!("Page {:?} - Coming Soon", page))
                .into_any_element(),
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_page = self.entities.tabs.read(cx).active_page;
        let content = self.get_or_create_page(active_page, cx);

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(DfcColors::background())
            .child(
                // Header
                self.header.clone(),
            )
            .child(
                // Main content area
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    .child(
                        // Sidebar
                        self.sidebar.clone(),
                    )
                    .child(
                        // Content
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .bg(DfcColors::content_bg())
                            .child(content),
                    ),
            )
            .child(
                // Log panel
                self.log_panel.clone(),
            )
    }
}

/// Dispatch an AppEvent to the appropriate entity
fn dispatch_event(event: AppEvent, entities: &AppEntities, cx: &mut App) {
    match event {
        AppEvent::Log { level, message, timestamp } => {
            entities.logs.update(cx, |logs, cx| {
                logs.push(level, message, timestamp);
                cx.notify();
            });
        }
        AppEvent::ConnectionChanged { target, connected, detail } => {
            entities.connection.update(cx, |conn, cx| {
                conn.set_status(target, connected, detail);
                cx.notify();
            });
        }
        AppEvent::ConfigLoaded { config } => {
            entities.config.update(cx, |state, cx| {
                state.update_config(config);
                cx.notify();
            });
        }
        AppEvent::PropertiesUpdated { properties } => {
            entities.properties.update(cx, |state, cx| {
                state.update_properties(properties);
                cx.notify();
            });
        }
        AppEvent::EventsUpdated { events } => {
            entities.events.update(cx, |state, cx| {
                state.update_events(events);
                cx.notify();
            });
        }
        AppEvent::CommandResponse { request_id, response } => {
            entities.commands.update(cx, |state, cx| {
                state.set_response(request_id, response);
                cx.notify();
            });
        }
    }
}
