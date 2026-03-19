use std::collections::VecDeque;
use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use common::command::Command;
use common::constants::RECENT_GAMES_LIMIT;
use common::database::Database;
use common::display::Display;
use common::geom::{Alignment, Point, Rect};
use common::locale::Locale;
use common::platform::{DefaultPlatform, Key, KeyEvent, Platform};
use common::resources::Resources;
use common::stylesheet::{Stylesheet, StylesheetColor};
use common::view::{ButtonHint, ButtonHints, Image, ImageMode, Label, ScrollList, View};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;

use crate::consoles::ConsoleMapper;
use crate::entry::game::Game;
use crate::view::entry_list::{CoreSelection, MenuEntry};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecentsCarouselState {
    pub selected: usize,
}

#[derive(Debug)]
pub struct RecentsCarousel {
    rect: Rect,
    res: Resources,
    games: Vec<Game>,
    selected: usize,
    screenshot: Image,
    game_name: Label<String>,
    button_hints: ButtonHints<String>,
    menu: Option<ScrollList>,
    menu_entries: Vec<MenuEntry>,
    core: Option<CoreSelection>,
    dirty: bool,
}

impl RecentsCarousel {
    pub fn new(rect: Rect, res: Resources, state: RecentsCarouselState) -> Result<Self> {
        let Rect { x, y, w, .. } = rect;

        let games = Self::load_games(&res)?;
        let selected = state.selected.min(games.len().saturating_sub(1));

        let styles = res.get::<Stylesheet>();

        let mut button_hints = {
            let locale = res.get::<Locale>();
            ButtonHints::new(
                res.clone(),
                vec![],
                vec![
                    ButtonHint::new(
                        res.clone(),
                        Point::zero(),
                        Key::A,
                        locale.t("button-select"),
                        Alignment::Right,
                    ),
                    ButtonHint::new(
                        res.clone(),
                        Point::zero(),
                        Key::X,
                        locale.t("sort-search"),
                        Alignment::Right,
                    ),
                ],
            )
        };

        let button_hints_rect = button_hints.bounding_box(&styles);
        let label_height = styles.ui.margin_y * 2 + styles.ui.ui_font.size as i32;
        // Calculate screenshot height based on where button hints start
        let available_height = (button_hints_rect.y - y) as u32;
        let screenshot_height = available_height.saturating_sub(label_height as u32);

        let mut screenshot =
            Image::empty(Rect::new(x, y, w, screenshot_height), ImageMode::Contain);
        screenshot.set_border_radius(12);
        screenshot.set_alignment(Alignment::Center);

        let game_name = Label::new(
            Point::new(
                x + w as i32 / 2,
                y + screenshot_height as i32 + styles.ui.margin_y,
            ),
            String::new(),
            Alignment::Center,
            Some(w - (styles.ui.margin_x * 2) as u32),
        );

        drop(styles);

        let mut carousel = Self {
            rect,
            res: res.clone(),
            games,
            selected,
            screenshot,
            game_name,
            button_hints,
            menu: None,
            menu_entries: Vec::new(),
            core: None,
            dirty: true,
        };

        carousel.game_name.scroll(true);
        carousel.update_current_game()?;

        Ok(carousel)
    }

    pub fn load_or_new(
        rect: Rect,
        res: Resources,
        state: Option<RecentsCarouselState>,
    ) -> Result<Self> {
        let state = state.unwrap_or_default();
        Self::new(rect, res, state)
    }

    fn load_games(res: &Resources) -> Result<Vec<Game>> {
        let database = res.get::<Database>();
        let db_games = database.select_last_played(RECENT_GAMES_LIMIT)?;

        let mut games = Vec::new();

        for game in db_games {
            let extension = game
                .path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default()
                .to_owned();

            let image =
                crate::entry::lazy_image::LazyImage::from_path(&game.path, game.image.clone());

            games.push(Game {
                name: game.name.clone(),
                full_name: game.name,
                path: game.path,
                image,
                extension,
                core: game.core,
                rating: game.rating,
                release_date: game.release_date,
                developer: game.developer,
                publisher: game.publisher,
                genres: game.genres,
                favorite: game.favorite,
                screenshot_path: game.screenshot_path,
            });
        }

        Ok(games)
    }

    fn update_current_game(&mut self) -> Result<()> {
        if self.games.is_empty() {
            self.screenshot.set_path(None);
            self.game_name.set_text(String::new());
            return Ok(());
        }

        let game = &mut self.games[self.selected];

        self.screenshot.set_path(
            game.screenshot_path
                .clone()
                .or_else(|| game.image.image().map(Path::to_owned)),
        );
        self.screenshot.set_should_draw();
        self.game_name.set_text(if game.favorite {
            format!("♥ {}", game.name)
        } else {
            game.name.clone()
        });
        self.button_hints.set_should_draw();

        self.dirty = true;
        Ok(())
    }

    pub fn save(&self) -> RecentsCarouselState {
        RecentsCarouselState { selected: 0 }
    }

    fn navigate_up(&mut self) -> Result<()> {
        if self.selected > 0 {
            self.selected -= 1;
            self.update_current_game()?;
        }
        Ok(())
    }

    fn navigate_down(&mut self) -> Result<()> {
        if self.selected < self.games.len().saturating_sub(1) {
            self.selected += 1;
            self.update_current_game()?;
        }
        Ok(())
    }

    async fn launch_game(&mut self, commands: Sender<Command>) -> Result<()> {
        if let Some(game) = self.games.get_mut(self.selected) {
            let command =
                self.res
                    .get::<ConsoleMapper>()
                    .launch_game(&self.res.get(), game, false)?;
            if let Some(cmd) = command {
                commands.send(cmd).await?;
            }
        }
        Ok(())
    }

    fn open_menu(&mut self) -> Result<()> {
        let Some(game) = self.games.get(self.selected) else {
            return Ok(());
        };

        let Rect { x, y, w, h } = self.rect;
        let styles = self.res.get::<Stylesheet>();
        let locale = self.res.get::<Locale>();

        let mut entries = vec![
            MenuEntry::Favorite(game.favorite),
            MenuEntry::Launch(None),
            MenuEntry::Reset,
            MenuEntry::RemoveFromRecents,
            MenuEntry::RepopulateDatabase,
        ];

        let cores = self
            .res
            .get::<ConsoleMapper>()
            .get_console(&game.path)
            .map(|c| c.cores.clone())
            .unwrap_or_default();

        if !cores.is_empty() {
            let core = game.core.to_owned().unwrap_or_else(|| cores[0].clone());
            let i = cores.iter().position(|c| c == &core).unwrap_or_default();

            if let MenuEntry::Launch(ref mut launch_core) = entries[1] {
                let console_mapper = self.res.get::<ConsoleMapper>();
                *launch_core = Some(console_mapper.get_core_name(&core));
            }

            self.core = Some(CoreSelection { core: i, cores });
        } else {
            self.core = None;
        }

        let height = entries.len() as u32
            * (styles.ui.ui_font.size + styles.ui.list_margin as u32 + styles.ui.padding_y as u32);

        let mut menu = ScrollList::new(
            self.res.clone(),
            Rect::new(
                x + styles.ui.margin_x + (w as i32 - styles.ui.margin_x * 2) / 6,
                (y + h as i32 - height as i32) / 2,
                (w - styles.ui.margin_x as u32 * 2) * 2 / 3,
                height,
            ),
            entries.iter().map(|e| e.text(&locale)).collect(),
            Alignment::Left,
            styles.ui.ui_font.size + styles.ui.padding_y as u32,
        );
        menu.set_background_color(Some(StylesheetColor::BackgroundHighlightBlend));
        self.menu = Some(menu);
        self.menu_entries = entries;

        Ok(())
    }
}

#[async_trait(?Send)]
impl View for RecentsCarousel {
    fn draw(
        &mut self,
        display: &mut <DefaultPlatform as Platform>::Display,
        styles: &Stylesheet,
    ) -> Result<bool> {
        let mut drawn = false;

        if let Some(menu) = &mut self.menu {
            if menu.should_draw() {
                let mut rect = menu.bounding_box(styles);
                rect.y -= styles.ui.margin_x;
                rect.h += styles.ui.margin_x as u32 * 2;
                rect.x -= styles.ui.margin_x * 2;
                rect.w += styles.ui.margin_x as u32 * 4;
                rect = rect.intersection(&display.bounding_box());

                let radius = (styles.ui.ui_font.size + styles.ui.margin_y as u32) / 2;
                common::display::fill_rounded_rect(
                    &mut display.pixmap_mut(),
                    rect,
                    radius,
                    StylesheetColor::BackgroundHighlightBlend.to_color(styles),
                );

                menu.set_should_draw();
                menu.draw(display, styles)?;
                drawn = true;
            }
            return Ok(drawn);
        }

        if self.dirty {
            display.load(self.rect)?;
            self.dirty = false;
            drawn = true;
        }

        if self.screenshot.should_draw() {
            drawn |= self.screenshot.draw(display, styles)?;
        }

        if self.games.is_empty() {
            let locale = self.res.get::<Locale>();
            let mut empty_label = Label::new(
                Point::new(
                    self.rect.x + self.rect.w as i32 / 2,
                    self.rect.y + self.rect.h as i32 / 2,
                ),
                locale.t("no-recent-games"),
                Alignment::Center,
                None,
            );
            drawn |= empty_label.draw(display, styles)?;
        } else if self.game_name.should_draw() {
            drawn |= self.game_name.draw(display, styles)?;
        }

        if self.button_hints.should_draw() {
            drawn |= self.button_hints.draw(display, styles)?;
        }

        Ok(drawn)
    }

    fn should_draw(&self) -> bool {
        self.menu
            .as_ref()
            .is_some_and(common::view::View::should_draw)
            || self.dirty
            || self.screenshot.should_draw()
            || self.game_name.should_draw()
            || self.button_hints.should_draw()
    }

    fn set_should_draw(&mut self) {
        self.dirty = true;
        if let Some(menu) = self.menu.as_mut() {
            menu.set_should_draw();
        }
        self.screenshot.set_should_draw();
        self.game_name.set_should_draw();
        self.button_hints.set_should_draw();
    }

    async fn handle_key_event(
        &mut self,
        event: KeyEvent,
        commands: Sender<Command>,
        bubble: &mut VecDeque<Command>,
    ) -> Result<bool> {
        if let Some(menu) = self.menu.as_mut() {
            match event {
                KeyEvent::Pressed(Key::Left) => {
                    if let Some(core) = self.core.as_mut() {
                        let selected = &mut self.menu_entries[menu.selected()];
                        if let MenuEntry::Launch(launch_core) = selected {
                            core.core = core.core.saturating_sub(1);
                            let console_mapper = self.res.get::<ConsoleMapper>();
                            *launch_core =
                                Some(console_mapper.get_core_name(&core.cores[core.core]));
                            menu.set_item(menu.selected(), selected.text(&self.res.get()));
                        }
                    }
                    Ok(true)
                }
                KeyEvent::Pressed(Key::Right) => {
                    if let Some(core) = self.core.as_mut() {
                        let selected = &mut self.menu_entries[menu.selected()];
                        if let MenuEntry::Launch(launch_core) = selected {
                            core.core = (core.core + 1).min(core.cores.len() - 1);
                            let console_mapper = self.res.get::<ConsoleMapper>();
                            *launch_core =
                                Some(console_mapper.get_core_name(&core.cores[core.core]));
                            menu.set_item(menu.selected(), selected.text(&self.res.get()));
                        }
                    }
                    Ok(true)
                }
                KeyEvent::Pressed(Key::Select | Key::B) => {
                    self.menu = None;
                    commands.send(Command::Redraw).await?;
                    Ok(true)
                }
                KeyEvent::Pressed(Key::A) => {
                    let selected = &self.menu_entries[menu.selected()];
                    match selected {
                        MenuEntry::Favorite(_) => {
                            if let Some(game) = self.games.get_mut(self.selected) {
                                game.favorite = !game.favorite;
                                self.res
                                    .get::<Database>()
                                    .set_favorite(&game.path, game.favorite)?;
                                self.update_current_game()?;
                            }
                            commands.send(Command::Redraw).await?;
                        }
                        MenuEntry::Launch(_) => {
                            if let Some(core) = self.core.as_ref() {
                                if let Some(game) = self.games.get_mut(self.selected) {
                                    let db = self.res.get::<Database>();
                                    let core_name = &core.cores[core.core];
                                    db.set_core(&game.path, core_name)?;
                                    game.core = Some(core_name.to_string());
                                }
                            }
                            self.core = None;
                            self.launch_game(commands).await?;
                        }
                        MenuEntry::Reset => {
                            if let Some(game) = self.games.get_mut(self.selected) {
                                let command = self.res.get::<ConsoleMapper>().launch_game(
                                    &self.res.get(),
                                    game,
                                    true,
                                )?;
                                if let Some(cmd) = command {
                                    commands.send(cmd).await?;
                                }
                            }
                            commands.send(Command::Redraw).await?;
                        }
                        MenuEntry::RemoveFromRecents => {
                            if let Some(game) = self.games.get(self.selected) {
                                if game.path.exists() {
                                    self.res.get::<Database>().reset_game(&game.path)?;
                                } else {
                                    self.res.get::<Database>().delete_game(&game.path)?;
                                }
                                self.games = Self::load_games(&self.res)?;
                                self.selected =
                                    self.selected.min(self.games.len().saturating_sub(1));
                                self.update_current_game()?;
                                commands.send(Command::Redraw).await?;
                            }
                        }
                        MenuEntry::RepopulateDatabase => {
                            commands.send(Command::Redraw).await?;
                            #[cfg(not(feature = "miyoo"))]
                            {
                                let message =
                                    self.res.get::<Locale>().t("populating-database");
                                commands.send(Command::Toast(message, None)).await?;
                            }
                            commands.send(Command::PopulateDb).await?;
                            #[cfg(not(feature = "miyoo"))]
                            {
                                commands
                                    .send(Command::Toast(
                                        String::new(),
                                        Some(std::time::Duration::ZERO),
                                    ))
                                    .await?;
                            }
                            commands.send(Command::Redraw).await?;
                        }
                    }
                    self.menu = None;
                    Ok(true)
                }
                _ => menu.handle_key_event(event, commands, bubble).await,
            }
        } else {
            match event {
                KeyEvent::Pressed(Key::Up) | KeyEvent::Autorepeat(Key::Up) => {
                    self.navigate_up()?;
                    Ok(true)
                }
                KeyEvent::Pressed(Key::Down) | KeyEvent::Autorepeat(Key::Down) => {
                    self.navigate_down()?;
                    Ok(true)
                }
                KeyEvent::Pressed(Key::A) => {
                    self.launch_game(commands).await?;
                    Ok(true)
                }
                KeyEvent::Pressed(Key::X) => {
                    commands.send(Command::StartSearch).await?;
                    Ok(true)
                }
                KeyEvent::Pressed(Key::Select) => {
                    self.open_menu()?;
                    Ok(true)
                }
                _ => Ok(false),
            }
        }
    }

    fn children(&self) -> Vec<&dyn View> {
        vec![&self.screenshot, &self.game_name, &self.button_hints]
    }

    fn children_mut(&mut self) -> Vec<&mut dyn View> {
        vec![
            &mut self.screenshot,
            &mut self.game_name,
            &mut self.button_hints,
        ]
    }

    fn bounding_box(&mut self, _styles: &Stylesheet) -> Rect {
        self.rect
    }

    fn set_position(&mut self, point: Point) {
        self.rect.x = point.x;
        self.rect.y = point.y;
    }
}
