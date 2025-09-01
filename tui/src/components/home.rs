use color_eyre::Result;
use crossterm::event::KeyCode;
use layout::Flex;
use ratatui::{prelude::*, widgets::*};
use style::palette::tailwind::SLATE;
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::{action::Action, app::Mode, config::Config};

fn center(area: Rect, horizontal: Constraint, vertical: Constraint) -> Rect {
    let [area] = Layout::horizontal([horizontal])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([vertical]).flex(Flex::Center).areas(area);
    area
}

#[derive(Default)]
pub struct Home {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    list: MenuList,
    mode: Mode,
    enabled: bool,
}

#[derive(Debug, Clone, Default)]
struct MenuList {
    list_items: Vec<MenuListItem>,
    state: ListState,
}

#[derive(Debug, Clone, Default)]
struct MenuListItem {
    mode: Mode,
}

const SELECTED_STYLE: Style = Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD);

impl FromIterator<Mode> for MenuList {
    fn from_iter<I: IntoIterator<Item = Mode>>(iter: I) -> Self {
        let items = iter.into_iter().map(MenuListItem::new).collect();
        let state = ListState::default();
        Self {
            list_items: items,
            state,
        }
    }
}
impl MenuListItem {
    fn new(mode: Mode) -> Self {
        Self { mode }
    }
}

impl MenuList {
    fn select_none(&mut self) {
        self.state.select(None);
    }

    fn select_next(&mut self) {
        self.state.select_next();
    }
    fn select_previous(&mut self) {
        self.state.select_previous();
    }

    fn select_first(&mut self) {
        self.state.select_first();
    }

    fn select_last(&mut self) {
        self.state.select_last();
    }
}

impl Home {
    pub fn new() -> Self {
        Home {
            list: MenuList::from_iter(vec![Mode::Add, Mode::QuickAdd, Mode::Toggle, Mode::List]),
            mode: Mode::Home,
            enabled: true,
            ..Default::default()
        }
    }
}

impl Component for Home {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }
    fn get_mode(&self) -> Mode {
        self.mode
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tick => {}
            Action::Render => {
                // add any logic here that should run on every render
            }
            Action::Mode(mode) => {
                self.enabled = mode == Mode::Home;
            }
            _ => {}
        }
        Ok(None)
    }
    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Result<Option<Action>> {
        if !self.enabled {
            return Ok(None);
        }
        match key.code {
            KeyCode::Char('h') | KeyCode::Left => self.list.select_none(),
            KeyCode::Char('j') | KeyCode::Down => self.list.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.list.select_previous(),
            KeyCode::Char('g') | KeyCode::Home => self.list.select_first(),
            KeyCode::Char('G') | KeyCode::End => self.list.select_last(),
            KeyCode::Enter => {
                self.command_tx.as_ref().unwrap().send(Action::Mode(
                    self.list.list_items[self.list.state.selected().unwrap_or(0)].mode,
                ))?;
                return Ok(None);
            }
            KeyCode::Char('q') => return Ok(Some(Action::Quit)),
            _ => {}
        };
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let items: Vec<ListItem> = self.list.list_items.iter().map(ListItem::from).collect();
        let list = List::new(items)
            .highlight_style(SELECTED_STYLE)
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always)
            .scroll_padding(5)
            .block(
                Block::new()
                    .padding(Padding::uniform(1))
                    .title_top(Line::raw("Modes").centered().bold()),
            );

        let center_area = center(
            area,
            Constraint::Percentage(15),
            Constraint::Length(7), // top and bottom border + content
        );
        frame.render_stateful_widget(list, center_area, &mut self.list.state);

        Ok(())
    }
}

impl From<&MenuListItem> for ListItem<'_> {
    fn from(value: &MenuListItem) -> Self {
        ListItem::new(value.mode.to_string())
    }
}
