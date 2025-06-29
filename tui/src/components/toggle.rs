use super::Component;
use crate::{action::Action, app::Mode, config::Config};
use color_eyre::Result;
use crossterm::event::KeyCode;
use modder::{
    Link, calc_sha512,
    cli::Source,
    metadata::Metadata,
    modrinth_wrapper::modrinth::{GetProject, VersionData},
};
use ratatui::{prelude::*, widgets::*};
use std::{fs, path::PathBuf};
use style::palette::tailwind::SLATE;
use tokio::sync::mpsc::UnboundedSender;
use tracing::error;
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

#[derive(Default)]
pub struct ToggleComponent {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    list: ToggleList,
    mode: Mode,
    enabled: bool,
    state: State,
    input: Input,
    throbber_state: throbber_widgets_tui::ThrobberState,
    dir: PathBuf,
}

#[derive(Debug, Clone, Default)]
struct ToggleList {
    filtered_items: Vec<ToggleListItem>,
    list_items: Vec<ToggleListItem>,
    state: ListState,
}

#[derive(Debug, Clone, Default, PartialEq)]
enum State {
    #[default]
    Normal,
    Search,
    Toggling,
}

#[derive(Debug, Clone, Default)]
struct ToggleListItem {
    name: String,
    source: Source,
    project_id: String,
    version: String,
    game_version: Option<String>,
    category: Option<String>,
    version_type: String,
    enabled: bool,
    path: String,
}

const SELECTED_STYLE: Style = Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD);

impl FromIterator<ToggleListItem> for ToggleList {
    fn from_iter<I: IntoIterator<Item = ToggleListItem>>(iter: I) -> Self {
        let items = iter.into_iter().collect();
        let state = ListState::default();
        Self {
            filtered_items: Vec::new(),
            list_items: items,
            state,
        }
    }
}

impl ToggleList {
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

impl ToggleComponent {
    pub async fn new(dir: PathBuf) -> Self {
        let dir_clone = dir.clone();
        let items = tokio::spawn(async move { get_mods(dir_clone.clone()).await }).await;
        let items = items.unwrap_or(Vec::new());

        ToggleComponent {
            list: ToggleList::from_iter(items),
            mode: Mode::Toggle,
            enabled: true,
            dir,
            ..Default::default()
        }
    }
    pub fn toggle_state(&mut self) {
        self.state = match self.state {
            State::Normal => State::Search,
            State::Search => State::Normal,
            State::Toggling => State::Normal,
        };
    }
}

impl Component for ToggleComponent {
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
                self.enabled = mode == self.mode;
                if self.enabled {
                    self.list.select_first();
                    let dir = self.dir.clone();
                    self.list.list_items =
                        futures::executor::block_on(async move { get_mods(dir).await });
                }
            }
            _ => {}
        }
        Ok(None)
    }
    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Result<Option<Action>> {
        if !self.enabled {
            return Ok(None);
        }
        if self.state == State::Search {
            match key.code {
                KeyCode::Tab | KeyCode::Esc => self.toggle_state(),
                KeyCode::Enter => {}
                _ => {
                    self.input.handle_event(&crossterm::event::Event::Key(key));
                    let val = self.input.value();
                    let filtered_items = self
                        .list
                        .list_items
                        .iter()
                        .filter(|item| item.name.to_lowercase().contains(&val.to_lowercase()))
                        .cloned()
                        .collect();
                    self.list.filtered_items = filtered_items;
                    self.list.state.select_first();
                }
            }
            return Ok(None);
        }
        match key.code {
            KeyCode::Char('h') | KeyCode::Left => self.list.select_none(),
            KeyCode::Char('j') | KeyCode::Down => self.list.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.list.select_previous(),
            KeyCode::Char('g') | KeyCode::Home => self.list.select_first(),
            KeyCode::Char('G') | KeyCode::End => self.list.select_last(),
            KeyCode::Char(' ') => {
                let idx = self.list.state.selected().unwrap();
                if self.list.filtered_items.is_empty() {
                    let mut item = self.list.list_items[idx].clone();
                    item.enabled = !item.enabled;
                    self.list.list_items[idx] = item;
                    return Ok(None);
                }
                self.list.filtered_items[idx].enabled = !self.list.filtered_items[idx].enabled;
            }
            KeyCode::Enter => {
                self.state = State::Toggling;
                for item in self.list.list_items.iter() {
                    let filename = item.path.split('/').last().unwrap();
                    let predicate = filename.contains("disabled");
                    if predicate && item.enabled {
                        let new_path = item.path.replace(".disabled", "");
                        let res = fs::rename(item.path.clone(), new_path);
                        if res.is_err() {
                            error!("Failed to rename file: {:?}", res.err());
                        }
                    }
                    if !predicate && !item.enabled {
                        let new_path = format!("{}.disabled", item.path);

                        let res = fs::rename(item.path.clone(), new_path);
                        if res.is_err() {
                            error!("Failed to rename file: {:?}", res.err());
                        }
                    }
                }
                self.state = State::Normal;
            }
            KeyCode::Char('q') => return Ok(Some(Action::Quit)),
            KeyCode::Esc => {
                self.command_tx.clone().unwrap().send(Action::ClearScreen)?;
                return Ok(Some(Action::Mode(Mode::Home)));
            }

            KeyCode::Char('/') => self.toggle_state(),
            _ => {}
        };
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let items: Vec<ListItem> = if self.list.filtered_items.is_empty() {
            self.list.list_items.iter().map(ListItem::from).collect()
        } else {
            self.list
                .filtered_items
                .iter()
                .map(ListItem::from)
                .collect()
        };
        let list = List::new(items)
            .highlight_style(SELECTED_STYLE)
            .highlight_symbol("> ")
            .highlight_spacing(HighlightSpacing::Always)
            .block(
                Block::new()
                    .borders(Borders::ALL)
                    .padding(Padding::uniform(1))
                    .border_type(BorderType::Rounded)
                    .title_top(Line::raw("Mods").centered().bold()),
            );

        let [top, center] =
            Layout::vertical([Constraint::Min(3), Constraint::Percentage(100)]).areas(area);
        let [left, right] =
            Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
                .areas(center);
        let [lt, lb] =
            Layout::vertical([Constraint::Percentage(100), Constraint::Min(3)]).areas(left);
        let top_text = Paragraph::new("List")
            .bold()
            .block(
                Block::default()
                    .padding(Padding::symmetric(1, 0))
                    .border_type(BorderType::Rounded)
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::White));
        let right_widget =
            if self.list.state.selected().is_some() && !self.list.list_items.is_empty() {
                let selected = self.list.state.selected().unwrap();
                let idx = selected.min(self.list.list_items.len() - 1);
                let item = if self.list.filtered_items.is_empty() {
                    &self.list.list_items[idx]
                } else {
                    &self.list.filtered_items[idx]
                };

                let name_span = Span::styled(
                    item.name.clone() + "  ",
                    Style::default().add_modifier(Modifier::BOLD),
                );
                let version_span = Span::styled(
                    item.version.clone(),
                    Style::default().add_modifier(Modifier::DIM),
                );
                let top_line = Line::from(vec![name_span, version_span]);
                let url = if item.source == Source::Modrinth {
                    format!("https://modrinth.com/mod/{}", item.project_id)
                } else {
                    format!("https://github.com/{}", item.project_id)
                };

                let new_link = Link::new(url.clone(), url);

                let lines = vec![
                    top_line,
                    Line::from(vec![
                        Span::styled("\tSource: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(item.source.to_string()),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            "\tVersion: ",
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(&item.version),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            "\tURL: ",
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(new_link.to_string()),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            "\tMod version: ",
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(format!("[{}]", item.version)),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            "\tGame versions: ",
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(item.game_version.clone().unwrap_or_else(|| "-".to_string())),
                    ]),
                    Line::from(vec![
                        Span::styled("\tLoaders: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw(item.category.clone().unwrap_or_else(|| "-".to_string())),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            "\tCategories: ",
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(item.category.clone().unwrap_or_else(|| "-".to_string())),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            "\tStatus: ",
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(item.version_type.clone()),
                    ]),
                    // Add more fields as needed
                ];
                let para = Paragraph::new(lines).alignment(Alignment::Left);
                para
            } else {
                Paragraph::new(Span::raw("No mod selected")).alignment(Alignment::Left)
            };
        let loader = throbber_widgets_tui::Throbber::default()
            .label("Toggling Mods")
            .throbber_set(throbber_widgets_tui::BRAILLE_SIX_DOUBLE);
        let right_widget = right_widget.block(
            Block::new()
                .borders(Borders::ALL)
                .padding(Padding::uniform(1))
                .title_top(Line::raw("info").centered().bold())
                .border_type(BorderType::Rounded),
        );
        let style = match self.state {
            State::Normal => Style::default(),
            State::Search => Color::Yellow.into(),
            State::Toggling => Style::default(),
        };
        let input = Paragraph::new(self.input.value())
            .style(style)
            .block(Block::bordered().title("Input"));
        match self.state {
            State::Toggling => {
                frame.render_stateful_widget(loader, lb, &mut self.throbber_state);
            }
            _ => {
                frame.render_widget(input, lb);
            }
        }
        frame.render_widget(top_text, top);
        frame.render_widget(right_widget, right);
        frame.render_stateful_widget(list, lt, &mut self.list.state);
        Ok(())
    }
}

impl From<&ToggleListItem> for ListItem<'_> {
    fn from(value: &ToggleListItem) -> Self {
        ListItem::new(value.format())
    }
}

#[allow(clippy::needless_lifetimes)]
impl<'a> ToggleListItem {
    fn format(&self) -> Line<'a> {
        let version_type_style = match self.version_type.to_uppercase().as_str() {
            "RELEASE" => Style::default().fg(Color::Green),
            "BETA" => Style::default().fg(Color::Yellow),
            "ALPHA" => Style::default().fg(Color::Red),
            "GITHUB" => Style::default().fg(Color::Cyan),
            _ => Style::default().fg(Color::Cyan),
        };
        let version_type_text = match self.version_type.to_uppercase().as_str() {
            "RELEASE" => "RELEASE",
            "BETA" => "BETA   ",
            "ALPHA" => "ALPHA  ",
            "GITHUB" => "GITHUB ",
            _ => "UNKNOWN",
        };
        let enabled_span = Span::styled(
            if self.enabled { "[x]" } else { "[ ]" }.to_string() + "  ",
            if self.enabled {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
                    .add_modifier(Modifier::DIM)
                    .fg(Color::White)
            },
        );
        let span = Span::styled(version_type_text.to_string() + "  ", version_type_style);
        let id_span = Span::styled(
            self.project_id.clone() + "  ",
            Style::default().add_modifier(Modifier::DIM),
        );
        let name = self.name.clone();
        let name_span = Span::styled(name.clone(), Style::default().add_modifier(Modifier::BOLD));
        if !self.enabled {
            return Line::from(vec![enabled_span, span, id_span, name_span])
                .style(Style::default().add_modifier(Modifier::DIM));
        }
        Line::from(vec![enabled_span, span, id_span, name_span])
    }
}

async fn get_mods(dir: PathBuf) -> Vec<ToggleListItem> {
    let files = fs::read_dir(dir).unwrap();

    let regex = regex::Regex::new(r#"\b\d+\.\d+(?:\.\d+)?(?:-(?:pre|rc)\d+)?\b"#);
    let mut output = Vec::new();
    let mut handles = Vec::new();
    for f in files {
        let regex = regex.clone();
        let handle = tokio::spawn(async move {
            if f.is_err() {
                return None;
            }
            let f = f.unwrap();
            let path = f.path();
            let extension = path
                .extension()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default();

            if extension != "jar" && extension != "disabled" {
                return None;
            }

            let path_str = path.to_str().unwrap_or_default().to_string();
            let hash = calc_sha512(&path_str);
            let enabled = !path_str.contains("disabled");
            let version_data = VersionData::from_hash(hash).await;
            if version_data.is_err() {
                let metadata = Metadata::get_all_metadata(path_str.clone().into());
                if metadata.is_err() {
                    error!(version_data = ?version_data, "Failed to get version data for {}", path_str);
                    return None;
                }
                let metadata = metadata.unwrap();
                let source = metadata.get("source").unwrap();
                if source.is_empty() {
                    error!(version_data = ?version_data, "Failed to get version data for {}", path_str);
                    return None;
                }
                let repo = metadata.get("repo").unwrap();
                let repo_name = repo.split('/').last().unwrap();
                let game_version = regex.unwrap().find(&path_str).unwrap().as_str().to_string();
                let out = ToggleListItem {
                    name: repo_name.to_string(),
                    source: Source::Github,
                    version: game_version,
                    game_version: None,
                    category: None,
                    version_type: "GITHUB".to_string(),
                    project_id: repo.to_string(),
                    enabled,
                    path: path_str.to_string(),
                };
                return Some(out);
            }
            let version_data = version_data.unwrap();
            let project = GetProject::from_id(&version_data.project_id).await?;

            let out = ToggleListItem {
                name: project.get_title(),
                source: Source::Modrinth,
                game_version: Some(
                    version_data
                        .get_game_versions()
                        .unwrap_or(Vec::new())
                        .join(", "),
                ),
                enabled,
                path: path_str.to_string(),
                version: version_data.get_version(),
                category: Some(project.get_categories().join(", ")),
                version_type: version_data.get_version_type(),
                project_id: version_data.project_id,
            };

            Some(out)
        });
        handles.push(handle);
    }
    for handle in handles {
        let out = match handle.await {
            Ok(out) => out,
            Err(_) => continue,
        };
        let Some(out) = out else {
            continue;
        };
        output.push(out);
    }
    output
}
