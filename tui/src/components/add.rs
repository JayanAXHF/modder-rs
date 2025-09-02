use super::Component;
use crate::{action::Action, app::Mode, config::Config};
use color_eyre::Result;
use crossterm::event::KeyCode;
use futures::{executor::block_on, lock::Mutex};
use modder::{
    MOD_LOADERS, ModLoader, calc_sha512,
    cli::{SOURCES, Source},
    curseforge_wrapper::{CurseForgeAPI, CurseForgeError},
    gh_releases::{GHReleasesAPI, get_mod_from_release},
    metadata::Metadata,
    modrinth_wrapper::modrinth::{self, GetProject, Mod, Modrinth, VersionData},
};
use ratatui::{prelude::*, widgets::*};
use std::{
    collections::HashSet,
    fmt::Debug,
    fs,
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::Arc,
};
use style::palette::tailwind::SLATE;
use throbber_widgets_tui::{Throbber, ThrobberState};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error, info};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;
use tui_logger::*;

#[derive(Default)]
pub struct AddComponent {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    list: CurrentModsList,
    mode: Mode,
    dir: PathBuf,
    enabled: bool,
    state: State,
    input: Input,
    source_list: SourceList,
    search_result_list: AddList,
    version_input: Input,
    selected_list_state: ListState,
    logger_state: TuiWidgetState,
    throbber_state: ThrobberState,
    loader_list: LoaderList,
}

#[derive(Debug, Clone, Default)]
struct SourceList {
    list_items: Vec<Source>,
    state: ListState,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
enum SearchResult {
    ModrinthMod(ModrinthAddListItem),
    Github(GithubAddListItem),
    CurseForgeMod(CurseForgeAddListItem),
}
impl Default for SearchResult {
    fn default() -> Self {
        Self::ModrinthMod(ModrinthAddListItem::default())
    }
}

#[derive(Debug, Clone, Default)]
struct CurrentModsList {
    filtered_items: Vec<CurrentModsListItem>,
    list_items: Vec<CurrentModsListItem>,
    state: ListState,
}

#[derive(Default, Clone)]
struct AddList {
    list_items: Vec<SearchResult>,
    state: ListState,
    selected_items: HashSet<SearchResult>,
}

trait AddListItem {
    fn get_name(&self) -> String;
}
trait Downloadable {
    async fn download(&self, dir: PathBuf) -> Result<()>;
}

#[derive(Debug, Clone, Default, PartialEq, Hash, Eq)]
pub struct ModrinthAddListItem {
    name: String,
    source: Source,
    project_id: String,
    version: String,
    game_version: String,
    slug: String,
    selected: bool,
    mod_loader: ModLoader,
}

#[derive(Debug, Clone, Default)]
pub struct GithubAddListItem {
    name: String,
    source: Source,
    repo: String,
    version: String,
    game_version: String,
    selected: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct CurseForgeAddListItem {
    name: String,
    source: Source,
    author: String,
    id: u32,
    game_version: String,
    version_id: u32,
    selected: bool,
    slug: String,
    thumbs_up_count: u32,
    loader: ModLoader,
}

#[derive(Debug, Clone, Default)]
pub struct LoaderList {
    list_items: Vec<ModLoader>,
    state: ListState,
}

impl Hash for GithubAddListItem {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.source.hash(state);
        self.repo.hash(state);
        self.version.hash(state);
        self.game_version.hash(state);
    }
}

impl PartialEq for GithubAddListItem {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.source == other.source
            && self.repo == other.repo
            && self.version == other.version
            && self.game_version == other.game_version
    }
}

impl Eq for GithubAddListItem {}

impl AddListItem for ModrinthAddListItem {
    fn get_name(&self) -> String {
        self.name.clone()
    }
}

impl SearchResult {
    fn get_is_selected(&self) -> bool {
        match self {
            SearchResult::ModrinthMod(mod_) => mod_.selected,
            SearchResult::Github(github) => github.selected,
            SearchResult::CurseForgeMod(curseforge) => curseforge.selected,
        }
    }
    fn toggle_selected(&mut self) {
        match self {
            SearchResult::ModrinthMod(mod_) => mod_.selected = !mod_.selected,
            SearchResult::Github(github) => github.selected = !github.selected,
            SearchResult::CurseForgeMod(curseforge) => curseforge.selected = !curseforge.selected,
        }
    }
}

impl Downloadable for ModrinthAddListItem {
    async fn download(&self, dir: PathBuf) -> Result<()> {
        debug!(game_version = ?&self.game_version);
        debug!(slug = ?&self.slug);
        debug!(mod_loader = ?&self.mod_loader);
        let version_data =
            Modrinth::get_version(&self.slug, &self.game_version, self.mod_loader.clone()).await;
        if let Some(version_data) = version_data {
            modrinth::download_file(
                &version_data.clone().files.unwrap()[0],
                &dir.to_string_lossy(),
            )
            .await;
            let mod_ = Mod {
                slug: self.slug.clone(),
                title: self.name.clone(),
            };
            let dependencies = Arc::new(Mutex::new(Vec::new()));
            Modrinth::download_dependencies(
                &mod_,
                &self.game_version,
                dependencies,
                &dir.to_string_lossy(),
                self.mod_loader.clone(),
            )
            .await;
        } else {
            error!(
                "Could not find version {} for {}",
                &self.game_version, &self.name
            );
        }
        Ok(())
    }
}
impl AddListItem for GithubAddListItem {
    fn get_name(&self) -> String {
        self.name.clone()
    }
}
impl AddListItem for CurseForgeAddListItem {
    fn get_name(&self) -> String {
        self.name.clone()
    }
}
impl Downloadable for GithubAddListItem {
    async fn download(&self, dir: PathBuf) -> Result<()> {
        let gh = GHReleasesAPI::new();
        let [owner, repo] = self.repo.split('/').collect::<Vec<&str>>()[..] else {
            error!("Invalid repo {}", self.repo);
            return Ok(());
        };
        let version_data = gh.get_releases(owner, repo).await;
        if let Ok(version_data) = version_data {
            let release = get_mod_from_release(&version_data, "fabric", &self.game_version).await;
            if let Ok(release) = release {
                let url = release.get_download_url().unwrap();
                let file_name = url.path_segments().unwrap().last().unwrap();
                let path = format!("{}{}", dir.to_string_lossy(), file_name);
                debug!(path = ?path);
                release
                    .download(path.clone().into(), self.repo.clone())
                    .await
                    .unwrap();
            } else {
                error!(err=?release.err().unwrap().to_string(), "Error finding or downloading mod");
            }
        } else {
            error!(
                "Could not find version {} for {}: {version_data:?}",
                &self.game_version, &self.name
            );
        }
        Ok(())
    }
}

impl Downloadable for CurseForgeAddListItem {
    async fn download(&self, dir: PathBuf) -> Result<()> {
        let cf = CurseForgeAPI::new(env!("CURSEFORGE_API_KEY").to_string());
        let files = cf
            .get_mod_files(self.id, &self.game_version, self.loader.clone())
            .await?;
        let file_id = files[0].id;
        let download_res = cf.download_mod(self.id, file_id, dir).await;
        if download_res.is_err() {
            return Err(download_res.err().unwrap().into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
enum State {
    #[default]
    Normal,
    Search,
    ToggleSource,
    SearchResultList,
    Downloading,
    VersionInput,
    SelectedList,
    ChangeLoader,
}

#[derive(Debug, Clone, Default)]
struct CurrentModsListItem {
    name: String,
    project_id: String,
    version_type: String,
    enabled: bool,
}

const SELECTED_STYLE: Style = Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD);

impl FromIterator<CurrentModsListItem> for CurrentModsList {
    fn from_iter<I: IntoIterator<Item = CurrentModsListItem>>(iter: I) -> Self {
        let items = iter.into_iter().collect();
        let state = ListState::default();
        Self {
            filtered_items: Vec::new(),
            list_items: items,
            state,
        }
    }
}
impl FromIterator<ModLoader> for LoaderList {
    fn from_iter<I: IntoIterator<Item = ModLoader>>(iter: I) -> Self {
        let items = iter.into_iter().collect();
        let state = ListState::default();
        Self {
            list_items: items,
            state,
        }
    }
}

impl CurrentModsList {
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
impl SourceList {
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
impl AddList {
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
    fn toggle_selected(&mut self) {
        let selected = self.state.selected().unwrap_or_default();
        let selected_item = self.list_items[selected].clone();
        match selected_item {
            SearchResult::ModrinthMod(mut mod_) => {
                mod_.selected = !mod_.selected;
                self.list_items[selected] = SearchResult::ModrinthMod(mod_.clone());
            }
            SearchResult::Github(mut github) => {
                github.selected = !github.selected;
                self.list_items[selected] = SearchResult::Github(github.clone());
            }
            SearchResult::CurseForgeMod(mut curseforge) => {
                curseforge.selected = !curseforge.selected;
                self.list_items[selected] = SearchResult::CurseForgeMod(curseforge.clone());
            }
        }
    }
}

impl LoaderList {
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

impl AddComponent {
    pub async fn new(dir: PathBuf) -> Self {
        let dir_clone = dir.clone();
        let items = tokio::spawn(async move { get_mods(dir_clone.clone()).await }).await;
        let items = items.unwrap_or(Vec::new());

        let source_list = SOURCES.clone();
        let loader_list = MOD_LOADERS.clone();
        AddComponent {
            list: CurrentModsList::from_iter(items),
            mode: Mode::Add,
            enabled: true,
            dir,
            source_list: SourceList {
                list_items: source_list,
                state: ListState::default(),
            },
            search_result_list: AddList {
                list_items: Vec::new(),
                state: ListState::default(),
                selected_items: HashSet::new(),
            },
            loader_list: LoaderList::from_iter(loader_list),

            ..Default::default()
        }
    }
    pub fn toggle_state(&mut self) {
        self.state = match self.state {
            State::Normal => State::Search,
            State::Search => State::Normal,
            _ => State::Normal,
        };
    }
    pub fn search(&mut self) -> Result<Option<Action>> {
        let version = self.version_input.value();
        let loader_idx = self.loader_list.state.selected().unwrap_or_default();

        let loader = self.loader_list.list_items[loader_idx].clone();
        if version.is_empty() {
            return Ok(None);
        }
        let search_term = self.input.value();
        let first_search = self.search_result_list.selected_items.is_empty();
        let search_results = match self.source_list.state.selected() {
            Some(selected) => {
                let selected = self.source_list.list_items[selected].clone();
                match selected {
                    Source::Modrinth => {
                        let mods =
                            futures::executor::block_on(Modrinth::search_mods(search_term, 100, 0));
                        debug!(search = ?search_term);
                        let hits = mods.hits;
                        debug!(search = ?hits);
                        hits.into_iter()
                            .map(|mod_| {
                                let mut mod_ = ModrinthAddListItem {
                                    name: mod_.title,
                                    source: Source::Modrinth,
                                    project_id: mod_.project_id,
                                    version: "".to_string(),
                                    game_version: version.to_string(),
                                    slug: mod_.slug,
                                    selected: true,
                                    mod_loader: loader.clone(),
                                };

                                let enabled = if first_search {
                                    false
                                } else {
                                    self.search_result_list
                                        .selected_items
                                        .contains(&SearchResult::ModrinthMod(mod_.clone()))
                                };

                                mod_.selected = enabled;
                                SearchResult::ModrinthMod(mod_)
                            })
                            .collect::<Vec<SearchResult>>()
                    }
                    Source::Github => {
                        let split = search_term.split('/').collect::<Vec<&str>>();
                        let repo = split.last().unwrap_or(&"");
                        let owner = split.first().unwrap_or(&"");
                        let releases = futures::executor::block_on(
                            GHReleasesAPI::new().get_releases(owner, repo),
                        );
                        if let Ok(releases) = releases {
                            releases
                                .into_iter()
                                .filter_map(|release| {
                                    if !release.name.clone()?.contains(version) {
                                        return None;
                                    };
                                    let mut github = GithubAddListItem {
                                        name: release.name?,
                                        source: Source::Github,
                                        repo: search_term.to_string(),
                                        version: release.tag_name.clone(),
                                        game_version: version.to_string(),
                                        selected: false,
                                    };
                                    let enabled = if first_search {
                                        false
                                    } else {
                                        self.search_result_list
                                            .selected_items
                                            .contains(&SearchResult::Github(github.clone()))
                                    };
                                    github.selected = enabled;
                                    Some(SearchResult::Github(github))
                                })
                                .collect::<Vec<SearchResult>>()
                        } else {
                            error!(err=?releases.err().unwrap().to_string(), "Error finding or downloading mod");
                            Vec::new()
                        }
                    }
                    Source::CurseForge => {
                        let version = self.version_input.value();
                        let loader_idx = self.loader_list.state.selected().unwrap_or_default();
                        let search = self.input.value();
                        let loader = self.loader_list.list_items[loader_idx].clone();
                        let cf = CurseForgeAPI::new(env!("CURSEFORGE_API_KEY").to_string());
                        info!(
                            "Searching curseforge for {}. This may take a few seconds",
                            search
                        );
                        let search_res =
                            block_on(cf.search_mods(version, loader.clone(), search, 30))?;
                        search_res
                            .into_iter()
                            .map(|mod_| {
                                let mut curseforge_add_list_item = CurseForgeAddListItem {
                                    name: mod_.name,
                                    source: Source::CurseForge,
                                    id: mod_.id,
                                    game_version: version.to_string(),
                                    version_id: mod_.main_file_id,
                                    selected: false,
                                    slug: mod_.slug,
                                    loader: loader.clone(),
                                    author: mod_
                                        .authors
                                        .iter()
                                        .map(|author| author.name.clone())
                                        .collect::<Vec<String>>()
                                        .join(", "),
                                    thumbs_up_count: mod_.thumbs_up_count,
                                };

                                let enabled = if first_search {
                                    false
                                } else {
                                    self.search_result_list.selected_items.contains(
                                        &SearchResult::CurseForgeMod(
                                            curseforge_add_list_item.clone(),
                                        ),
                                    )
                                };

                                curseforge_add_list_item.selected = enabled;
                                SearchResult::CurseForgeMod(curseforge_add_list_item)
                            })
                            .collect::<Vec<SearchResult>>()
                    }
                    #[allow(unreachable_patterns)]
                    _ => {
                        unreachable!();
                    }
                }
            }
            None => Vec::new(),
        };
        self.search_result_list.list_items = search_results;
        self.state = State::SearchResultList;
        Ok(None)
    }
}

impl ModrinthAddListItem {
    pub fn format(&self) -> Vec<Line<'static>> {
        let selected_indicator = if self.selected {
            Span::styled("[x] ", Style::default().fg(Color::Green))
        } else {
            Span::raw("[ ] ")
        };
        let padding = Span::raw("    ");
        vec![
            Line::from(vec![
                selected_indicator,
                Span::styled(
                    self.name.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
            ]),
            Line::from(vec![
                padding.clone(),
                Span::styled(
                    "Source: ".to_string(),
                    Style::default().add_modifier(Modifier::DIM),
                ),
                Span::styled(self.source.to_string(), Style::default().fg(Color::Blue)),
                Span::raw("  "),
                Span::styled(
                    "Game Version: ".to_string(),
                    Style::default().add_modifier(Modifier::DIM),
                ),
                Span::styled(
                    self.game_version.clone(),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(vec![
                padding,
                Span::styled(
                    "Slug: ".to_string(),
                    Style::default().add_modifier(Modifier::DIM),
                ),
                Span::styled(self.slug.clone(), Style::default().fg(Color::Gray)),
                Span::raw("\n"),
            ]),
        ]
    }
}
impl GithubAddListItem {
    pub fn format(&self) -> Vec<Line<'static>> {
        let selected_indicator = if self.selected {
            Span::styled("[x] ", Style::default().fg(Color::Green))
        } else {
            Span::raw("[ ] ")
        };
        let padding = Span::raw("    ");
        vec![
            Line::from(vec![
                selected_indicator,
                Span::styled(
                    self.name.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("v. {}", self.version),
                    Style::default().fg(Color::Green),
                ),
            ]),
            Line::from(vec![
                padding.clone(),
                Span::styled(
                    "Source: ".to_string(),
                    Style::default().add_modifier(Modifier::DIM),
                ),
                Span::styled(self.source.to_string(), Style::default().fg(Color::Blue)),
                Span::raw("  "),
                Span::styled(
                    "Game Version: ".to_string(),
                    Style::default().add_modifier(Modifier::DIM),
                ),
                Span::styled(
                    self.game_version.clone(),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(vec![
                padding,
                Span::styled(
                    "Repo: ".to_string(),
                    Style::default().add_modifier(Modifier::DIM),
                ),
                Span::styled(self.repo.clone(), Style::default().fg(Color::Gray)),
            ]),
        ]
    }
}

impl CurseForgeAddListItem {
    pub fn format(&self) -> Vec<Line<'static>> {
        let selected_indicator = if self.selected {
            Span::styled("[x] ", Style::default().fg(Color::Green))
        } else {
            Span::raw("[ ] ")
        };
        let padding = Span::raw("    ");
        vec![
            Line::from(vec![
                selected_indicator,
                Span::styled(
                    self.name.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::raw("by "),
                Span::styled(
                    self.author.clone(),
                    Style::default().add_modifier(Modifier::DIM),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("(Likes: {})", self.thumbs_up_count),
                    Style::default().add_modifier(Modifier::DIM),
                ),
            ]),
            Line::from(vec![
                padding.clone(),
                Span::styled(
                    "Source: ".to_string(),
                    Style::default().add_modifier(Modifier::DIM),
                ),
                Span::styled(self.source.to_string(), Style::default().fg(Color::Blue)),
                Span::raw("  "),
                Span::styled(
                    "Game Version: ".to_string(),
                    Style::default().add_modifier(Modifier::DIM),
                ),
                Span::styled(
                    self.game_version.clone(),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(vec![
                padding,
                Span::styled(
                    "Slug: ".to_string(),
                    Style::default().add_modifier(Modifier::DIM),
                ),
                Span::styled(self.slug.clone(), Style::default().fg(Color::Gray)),
                Span::raw("\n"),
            ]),
        ]
    }
}

impl Component for AddComponent {
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
            Action::Tick => {
                if self.state == State::Downloading {
                    self.throbber_state.calc_next()
                }
            }
            Action::Render => {
                // add any logic here that should run on every render
            }
            Action::Mode(mode) => {
                self.enabled = mode == self.mode;
                if self.enabled {
                    self.list.select_first();
                    self.source_list.select_first();
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
                KeyCode::Enter => {
                    self.search()?;
                }
                _ => {
                    self.input.handle_event(&crossterm::event::Event::Key(key));
                }
            }
            return Ok(None);
        }
        if self.state == State::ToggleSource {
            match key.code {
                KeyCode::Char('h') | KeyCode::Left => self.source_list.select_none(),
                KeyCode::Char('j') | KeyCode::Down => self.source_list.select_next(),
                KeyCode::Char('k') | KeyCode::Up => self.source_list.select_previous(),
                KeyCode::Char('g') | KeyCode::Home => self.source_list.select_first(),
                KeyCode::Char('G') | KeyCode::End => self.source_list.select_last(),
                KeyCode::Char('q') => return Ok(Some(Action::Quit)),
                KeyCode::Enter => {
                    if !self.version_input.value().is_empty()
                        && !self.input.value().is_empty()
                        && self.loader_list.state.selected().is_some()
                    {
                        self.search()?;
                    }
                }
                KeyCode::Esc => {
                    self.state = State::Normal;
                }
                _ => {}
            };
            return Ok(None);
        }
        if self.state == State::ChangeLoader {
            match key.code {
                KeyCode::Tab | KeyCode::Esc => self.state = State::Normal,
                KeyCode::Enter => {
                    if !self.version_input.value().is_empty() && !self.input.value().is_empty() {
                        self.search()?;
                        return Ok(None);
                    }

                    self.state = State::Search;
                }
                KeyCode::Char('h') | KeyCode::Left => self.loader_list.select_none(),
                KeyCode::Char('j') | KeyCode::Down => self.loader_list.select_next(),
                KeyCode::Char('k') | KeyCode::Up => self.loader_list.select_previous(),
                KeyCode::Char('g') | KeyCode::Home => self.loader_list.select_first(),
                KeyCode::Char('G') | KeyCode::End => self.loader_list.select_last(),
                KeyCode::Char('q') => return Ok(Some(Action::Quit)),
                _ => {}
            }
            return Ok(None);
        }
        if self.state == State::VersionInput {
            match key.code {
                KeyCode::Tab | KeyCode::Esc => self.state = State::Normal,
                KeyCode::Enter => {
                    if !self.version_input.value().is_empty()
                        && !self.input.value().is_empty()
                        && self.loader_list.state.selected().is_some()
                    {
                        self.search()?;
                        return Ok(None);
                    }

                    self.state = State::Search;
                }
                key => {
                    self.version_input
                        .handle_event(&crossterm::event::Event::Key(key.into()));
                }
            }
            return Ok(None);
        }
        if self.state == State::SearchResultList {
            match key.code {
                KeyCode::Char('h') | KeyCode::Left => self.search_result_list.select_none(),
                KeyCode::Char('j') | KeyCode::Down => self.search_result_list.select_next(),
                KeyCode::Char('k') | KeyCode::Up => self.search_result_list.select_previous(),
                KeyCode::Char('g') | KeyCode::Home => self.search_result_list.select_first(),
                KeyCode::Char('G') | KeyCode::End => self.search_result_list.select_last(),
                KeyCode::Char('q') => return Ok(Some(Action::Quit)),
                KeyCode::Char(' ') => {
                    self.search_result_list.toggle_selected();
                    let mut selected = self.search_result_list.list_items
                        [self.search_result_list.state.selected().unwrap_or_default()]
                    .clone();
                    if selected.get_is_selected() {
                        self.search_result_list.selected_items.insert(selected);
                        return Ok(None);
                    }
                    selected.toggle_selected();
                    self.search_result_list.selected_items.remove(&selected);
                }
                KeyCode::Esc => {
                    self.state = State::Normal;
                }
                KeyCode::Enter => {
                    let selected = self.search_result_list.selected_items.clone();
                    if selected.is_empty() {
                        info!("No mod selected");
                        return Ok(None);
                    }
                    info!("Downloading mods");
                    self.state = State::Downloading;
                    self.input.reset();
                    self.search_result_list.state.select(None);
                    self.search_result_list.selected_items.clear();

                    for selected in selected {
                        let dir = self.dir.clone();
                        match selected {
                            SearchResult::ModrinthMod(mod_) => {
                                info!("Downloading {}", mod_.get_name());
                                tokio::spawn(async move {
                                    let download_res = mod_.download(dir).await;
                                    match download_res {
                                        Ok(_) => {
                                            info!("Downloaded {}", mod_.get_name());
                                        }
                                        Err(err) => {
                                            error!(
                                                "Failed to download {}: {err:?}",
                                                mod_.get_name()
                                            );
                                        }
                                    }
                                });
                            }
                            SearchResult::Github(mod_) => {
                                info!("Downloading {}", mod_.get_name());
                                tokio::spawn(async move {
                                    let download_res = mod_.download(dir).await;
                                    match download_res {
                                        Ok(_) => {
                                            info!("Downloaded {}", mod_.get_name());
                                        }
                                        Err(err) => {
                                            error!(
                                                "Failed to download {}: {err:?}",
                                                mod_.get_name()
                                            );
                                        }
                                    }
                                });
                            }
                            SearchResult::CurseForgeMod(mod_) => {
                                info!("Downloading {}", mod_.get_name());
                                tokio::spawn(async move {
                                    let res = mod_.download(dir).await;
                                    match res {
                                        Ok(_) => {
                                            info!("Downloaded {}", mod_.get_name());
                                        }
                                        Err(err) => {
                                            error!(
                                                "Failed to download {}: {err:?}",
                                                mod_.get_name()
                                            );
                                        }
                                    }
                                });
                            }
                        }
                    }
                    let dir = self.dir.clone();
                    let items = futures::executor::block_on(async move { get_mods(dir).await });
                    info!("Finished downloading mods");
                    self.list.list_items = items;
                    self.state = State::Normal;
                }
                _ => {}
            };
            return Ok(None);
        }
        if self.state == State::SelectedList {
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => self.selected_list_state.select_next(),
                KeyCode::Char('k') | KeyCode::Up => self.selected_list_state.select_previous(),
                KeyCode::Char('g') | KeyCode::Home => self.selected_list_state.select_first(),
                KeyCode::Char('G') | KeyCode::End => self.selected_list_state.select_last(),
                KeyCode::Char('q') => return Ok(Some(Action::Quit)),
                KeyCode::Char('J') => {
                    self.state = State::VersionInput;
                }
                KeyCode::Esc => {
                    self.state = State::Normal;
                }
                _ => {}
            };
            return Ok(None);
        }
        match key.code {
            KeyCode::Char('h') | KeyCode::Left => self.list.select_none(),
            KeyCode::Char('j') | KeyCode::Down => self.list.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.list.select_previous(),
            KeyCode::Char('g') | KeyCode::Home => self.list.select_first(),
            KeyCode::Char('G') | KeyCode::End => self.list.select_last(),
            KeyCode::Char('S') => self.state = State::ToggleSource,
            KeyCode::Char('R') => self.state = State::SearchResultList,
            KeyCode::Char('V') => self.state = State::VersionInput,
            KeyCode::Char('q') => return Ok(Some(Action::Quit)),
            KeyCode::Char('/') => self.toggle_state(),
            KeyCode::Char('l') => self.state = State::SearchResultList,
            KeyCode::Char('J') | KeyCode::Char('s') => self.state = State::SelectedList,
            KeyCode::Char('L') => self.state = State::ChangeLoader,
            KeyCode::Esc => {
                self.command_tx.clone().unwrap().send(Action::ClearScreen)?;
                return Ok(Some(Action::Mode(Mode::Home)));
            }
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
        let search_results: Vec<ListItem> = self
            .search_result_list
            .list_items
            .iter()
            .map(SearchResult::to_list_item)
            .collect();
        let search_results_list_border = if self.state == State::SearchResultList {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        let search_results_list = List::new(search_results)
            .highlight_style(SELECTED_STYLE)
            .highlight_symbol("> ")
            .highlight_spacing(HighlightSpacing::Always)
            .block(
                Block::new()
                    .borders(Borders::ALL)
                    .padding(Padding::uniform(1))
                    .border_type(BorderType::Rounded)
                    .border_style(search_results_list_border)
                    .title_top(Line::raw("Search Results").centered().bold())
                    .title_bottom(Line::raw("Press `l` to select").right_aligned().bold()),
            );

        let search_results_list_border = if self.state == State::SelectedList {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        let selected_list = List::new(
            self.search_result_list
                .selected_items
                .iter()
                .map(SearchResult::to_list_item)
                .collect::<Vec<ListItem>>(),
        )
        .highlight_style(SELECTED_STYLE)
        .highlight_symbol("> ")
        .highlight_spacing(HighlightSpacing::Always)
        .block(
            Block::new()
                .borders(Borders::ALL)
                .padding(Padding::uniform(1))
                .border_type(BorderType::Rounded)
                .border_style(search_results_list_border)
                .title_top(Line::raw("Selected").centered().bold())
                .title_bottom(
                    Line::raw("Press `J` or `s` to select")
                        .right_aligned()
                        .bold(),
                ),
        );
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
        let [lt, lm, lb] = Layout::vertical([
            Constraint::Min(3),
            Constraint::Min(3),
            Constraint::Percentage(100),
        ])
        .areas(left);
        let [lb_1, lb_2] = Layout::vertical(Constraint::from_percentages([50, 50])).areas(lb);
        let [lm1, lm2] = Layout::horizontal(Constraint::from_percentages([70, 30])).areas(lm);
        let top_text = Paragraph::new("Add Mods")
            .bold()
            .block(
                Block::default()
                    .padding(Padding::symmetric(1, 0))
                    .border_type(BorderType::Rounded)
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::White));
        let style = match self.state {
            State::Search => Color::Yellow.into(),
            _ => Style::default(),
        };
        let input = Paragraph::new(self.input.value()).style(style).block(
            Block::bordered()
                .title("Search")
                .border_type(BorderType::Rounded)
                .title_bottom(Line::raw("Press `/` to select").right_aligned().bold()),
        );
        let version_input_style = match self.state {
            State::VersionInput => Color::Yellow.into(),
            _ => Style::default(),
        };
        let version_input = Paragraph::new(self.version_input.value())
            .style(version_input_style)
            .block(
                Block::bordered()
                    .title("Version")
                    .border_type(BorderType::Rounded)
                    .title_bottom(Line::raw("Press `V` to select").right_aligned().bold()),
            );
        let [ltl, ltr] =
            Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)]).areas(lt);
        let source_list_style = match self.state {
            State::ToggleSource => Color::Yellow.into(),
            _ => Style::default(),
        };

        let source_list = List::new(self.source_list.list_items.iter().map(|source| {
            let val = match source {
                Source::Modrinth => "MR",
                Source::Github => "GH",
                Source::CurseForge => "CF",
            };
            ListItem::new(val.to_string()).style(Style::default().fg(Color::Yellow))
        }))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title_top("Source")
                .border_style(source_list_style),
        );
        let loader_list_style = match self.state {
            State::ChangeLoader => Color::Yellow.into(),
            _ => Style::default(),
        };
        let loader_list = List::new(self.loader_list.list_items.iter().map(|loader| {
            let val = match loader {
                ModLoader::Forge => "Forge",
                ModLoader::Fabric => "Fabric",
                ModLoader::Quilt => "Quilt",
                ModLoader::NeoForge => "NeoForge",
                ModLoader::Cauldron => "Cauldron",
                ModLoader::LiteLoader => "LiteLoader",
                ModLoader::Any => "Any",
            };
            ListItem::new(val.to_string()).style(Style::default().fg(Color::Yellow))
        }))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title_top("Loader")
                .border_style(loader_list_style),
        );
        let [right_top, right_bottom] =
            Layout::vertical(Constraint::from_percentages([70, 30])).areas(right);

        let log_widget = TuiLoggerWidget::default()
            .style_error(Style::default().fg(Color::Red))
            .style_debug(Style::default().fg(Color::Cyan))
            .style_warn(Style::default().fg(Color::Yellow))
            .style_trace(Style::default().fg(Color::Magenta))
            .style_info(Style::default().fg(Color::Green))
            .output_separator(':')
            .output_timestamp(Some("%H:%M:%S".to_string()))
            .output_level(Some(TuiLoggerLevelOutput::Long))
            .output_target(false)
            .output_file(false)
            .output_line(false)
            .state(&self.logger_state)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title_top(Line::raw("Log").centered().bold()),
            );
        let throbber = Throbber::default().throbber_set(throbber_widgets_tui::BRAILLE_SIX_DOUBLE);
        let throbber_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded);
        let throbber_block_inner = throbber_block.inner(ltl);

        if self.state == State::Downloading {
            frame.render_stateful_widget(throbber, throbber_block_inner, &mut self.throbber_state);
        } else {
            frame.render_widget(input, ltl);
        }
        frame.render_stateful_widget(source_list, ltr, &mut self.source_list.state);
        frame.render_stateful_widget(loader_list, lm2, &mut self.loader_list.state);
        frame.render_stateful_widget(
            search_results_list,
            right_top,
            &mut self.search_result_list.state,
        );
        frame.render_widget(log_widget, right_bottom);
        frame.render_stateful_widget(selected_list, lb_1, &mut self.selected_list_state);
        frame.render_stateful_widget(list, lb_2, &mut self.list.state);
        frame.render_widget(top_text, top);
        frame.render_widget(version_input, lm1);

        Ok(())
    }
}

impl From<&CurrentModsListItem> for ListItem<'_> {
    fn from(value: &CurrentModsListItem) -> Self {
        ListItem::new(value.format())
    }
}

impl From<&CurseForgeAddListItem> for ListItem<'_> {
    fn from(value: &CurseForgeAddListItem) -> Self {
        ListItem::new(value.format())
    }
}

impl SearchResult {
    pub fn to_list_item(&self) -> ListItem<'static> {
        match self {
            SearchResult::ModrinthMod(mod_) => ListItem::new(mod_.format()),
            SearchResult::Github(github) => ListItem::new(github.format()),
            SearchResult::CurseForgeMod(curseforge) => ListItem::new(curseforge.format()),
        }
    }
}

#[allow(clippy::needless_lifetimes)]
impl<'a> CurrentModsListItem {
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

async fn get_mods(dir: PathBuf) -> Vec<CurrentModsListItem> {
    let files = fs::read_dir(dir).unwrap();
    let mut output = Vec::new();
    let mut handles = Vec::new();
    for f in files {
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
                let out = CurrentModsListItem {
                    name: repo_name.to_string(),
                    version_type: "GITHUB".to_string(),
                    project_id: repo_name.to_string(),
                    enabled,
                };
                return Some(out);
            }
            let version_data = version_data.unwrap();
            let project = GetProject::from_id(&version_data.project_id).await?;

            let out = CurrentModsListItem {
                name: project.get_title(),
                enabled,
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
