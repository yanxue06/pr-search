use std::io;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

use crate::embedding::ModelManager;
use crate::index::{IndexStorage, SemanticIndex};
use crate::search::{SearchEngine, SearchFilter, SearchResult};

use super::ui;

/// The TUI application state.
pub struct App {
    /// Current search query input
    pub query: String,
    /// Search results
    pub results: Vec<SearchResult>,
    /// Currently selected result index
    pub selected: usize,
    /// Status message displayed at the bottom
    pub status: String,
    /// Whether the app should quit
    pub should_quit: bool,
    /// The loaded semantic index
    index: SemanticIndex,
    /// The embedding model manager
    model: ModelManager,
    /// Whether the model is loaded
    model_loaded: bool,
    /// Current search filter
    pub filter: SearchFilter,
    /// Whether the input is focused (vs result list)
    pub input_focused: bool,
    /// Cursor position in the query string
    pub cursor_pos: usize,
}

impl App {
    /// Create a new App with a loaded index.
    pub fn new(storage: &IndexStorage, model: ModelManager) -> Result<Self> {
        let index = storage.load()?;
        let status = format!(
            "Loaded {} PRs from {} | Type to search, Enter to submit, Tab to switch focus",
            index.len(),
            index.repo
        );

        Ok(Self {
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            status,
            should_quit: false,
            index,
            model,
            model_loaded: false,
            filter: SearchFilter::default(),
            input_focused: true,
            cursor_pos: 0,
        })
    }

    /// Run the TUI event loop.
    pub fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.event_loop(&mut terminal);

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

        result
    }

    /// Main event loop.
    fn event_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        loop {
            terminal.draw(|frame| ui::draw(frame, self))?;

            if self.should_quit {
                break;
            }

            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // Global shortcuts
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    self.should_quit = true;
                    continue;
                }

                if self.input_focused {
                    self.handle_input_key(key.code);
                } else {
                    self.handle_list_key(key.code);
                }
            }
        }
        Ok(())
    }

    /// Handle key events when input is focused.
    fn handle_input_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char(c) => {
                self.query.insert(self.cursor_pos, c);
                self.cursor_pos += 1;
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    self.query.remove(self.cursor_pos);
                }
            }
            KeyCode::Left => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
            }
            KeyCode::Right => {
                if self.cursor_pos < self.query.len() {
                    self.cursor_pos += 1;
                }
            }
            KeyCode::Home => {
                self.cursor_pos = 0;
            }
            KeyCode::End => {
                self.cursor_pos = self.query.len();
            }
            KeyCode::Enter => {
                self.execute_search();
            }
            KeyCode::Tab => {
                if !self.results.is_empty() {
                    self.input_focused = false;
                }
            }
            KeyCode::Esc => {
                self.should_quit = true;
            }
            _ => {}
        }
    }

    /// Handle key events when result list is focused.
    fn handle_list_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < self.results.len() {
                    self.selected += 1;
                }
            }
            KeyCode::Home => {
                self.selected = 0;
            }
            KeyCode::End => {
                if !self.results.is_empty() {
                    self.selected = self.results.len() - 1;
                }
            }
            KeyCode::Enter | KeyCode::Char('o') => {
                self.open_selected_pr();
            }
            KeyCode::Tab => {
                self.input_focused = true;
            }
            KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Char('/') => {
                self.input_focused = true;
            }
            _ => {}
        }
    }

    /// Execute a search with the current query.
    fn execute_search(&mut self) {
        if self.query.trim().is_empty() {
            self.status = "Enter a search query".into();
            return;
        }

        // Load model on first search
        if !self.model_loaded {
            self.status = "Loading embedding model...".into();
            if let Err(e) = self.model.load() {
                self.status = format!("Failed to load model: {e}");
                return;
            }
            self.model_loaded = true;
        }

        self.status = format!("Searching for \"{}\"...", self.query);

        // Generate query embedding
        let embedding = match self.model.embed(&self.query) {
            Ok(e) => e,
            Err(e) => {
                self.status = format!("Embedding failed: {e}");
                return;
            }
        };

        // Search
        match SearchEngine::search(&self.index, &embedding, &self.filter, 50) {
            Ok(results) => {
                let count = results.len();
                self.results = results;
                self.selected = 0;
                self.input_focused = false;
                self.status = format!("Found {} results for \"{}\"", count, self.query);
            }
            Err(e) => {
                self.results.clear();
                self.status = format!("Search error: {e}");
            }
        }
    }

    /// Open the selected PR in the default browser.
    fn open_selected_pr(&mut self) {
        if let Some(result) = self.results.get(self.selected) {
            let url = result.html_url.clone();
            match open::that(&url) {
                Ok(_) => {
                    self.status = format!("Opened PR #{} in browser", result.number);
                }
                Err(e) => {
                    self.status = format!("Failed to open browser: {e}. URL: {url}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_input_key_char() {
        let mut app = make_test_app();
        app.handle_input_key(KeyCode::Char('h'));
        app.handle_input_key(KeyCode::Char('i'));
        assert_eq!(app.query, "hi");
        assert_eq!(app.cursor_pos, 2);
    }

    #[test]
    fn test_handle_input_key_backspace() {
        let mut app = make_test_app();
        app.query = "hello".into();
        app.cursor_pos = 5;
        app.handle_input_key(KeyCode::Backspace);
        assert_eq!(app.query, "hell");
        assert_eq!(app.cursor_pos, 4);
    }

    #[test]
    fn test_handle_input_key_backspace_at_start() {
        let mut app = make_test_app();
        app.query = "hello".into();
        app.cursor_pos = 0;
        app.handle_input_key(KeyCode::Backspace);
        assert_eq!(app.query, "hello"); // unchanged
    }

    #[test]
    fn test_handle_input_key_left_right() {
        let mut app = make_test_app();
        app.query = "abc".into();
        app.cursor_pos = 3;
        app.handle_input_key(KeyCode::Left);
        assert_eq!(app.cursor_pos, 2);
        app.handle_input_key(KeyCode::Right);
        assert_eq!(app.cursor_pos, 3);
    }

    #[test]
    fn test_handle_input_key_home_end() {
        let mut app = make_test_app();
        app.query = "hello world".into();
        app.cursor_pos = 5;
        app.handle_input_key(KeyCode::Home);
        assert_eq!(app.cursor_pos, 0);
        app.handle_input_key(KeyCode::End);
        assert_eq!(app.cursor_pos, 11);
    }

    #[test]
    fn test_handle_input_key_esc_quits() {
        let mut app = make_test_app();
        app.handle_input_key(KeyCode::Esc);
        assert!(app.should_quit);
    }

    #[test]
    fn test_handle_list_key_navigation() {
        let mut app = make_test_app();
        app.results = vec![
            make_result(1, 0.9),
            make_result(2, 0.8),
            make_result(3, 0.7),
        ];
        app.selected = 0;

        app.handle_list_key(KeyCode::Down);
        assert_eq!(app.selected, 1);
        app.handle_list_key(KeyCode::Down);
        assert_eq!(app.selected, 2);
        app.handle_list_key(KeyCode::Down); // at end, shouldn't go further
        assert_eq!(app.selected, 2);
        app.handle_list_key(KeyCode::Up);
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn test_handle_list_key_vim_navigation() {
        let mut app = make_test_app();
        app.results = vec![make_result(1, 0.9), make_result(2, 0.8)];
        app.selected = 0;

        app.handle_list_key(KeyCode::Char('j'));
        assert_eq!(app.selected, 1);
        app.handle_list_key(KeyCode::Char('k'));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn test_handle_list_key_tab_switches_focus() {
        let mut app = make_test_app();
        app.input_focused = false;
        app.handle_list_key(KeyCode::Tab);
        assert!(app.input_focused);
    }

    #[test]
    fn test_handle_list_key_slash_switches_to_input() {
        let mut app = make_test_app();
        app.input_focused = false;
        app.handle_list_key(KeyCode::Char('/'));
        assert!(app.input_focused);
    }

    #[test]
    fn test_empty_query_shows_status() {
        let mut app = make_test_app();
        app.query = "  ".into();
        app.execute_search();
        assert!(app.status.contains("Enter a search query"));
    }

    // Helper functions for tests
    fn make_test_app() -> App {
        App {
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            status: "Test".into(),
            should_quit: false,
            index: SemanticIndex::new("test/repo", false),
            model: ModelManager::new(std::path::PathBuf::from("/tmp/nonexistent")),
            model_loaded: false,
            filter: SearchFilter::default(),
            input_focused: true,
            cursor_pos: 0,
        }
    }

    fn make_result(number: u64, score: f32) -> SearchResult {
        SearchResult {
            number,
            title: format!("PR #{number}"),
            author: "test".into(),
            state: "open".into(),
            html_url: format!("https://github.com/o/r/pull/{number}"),
            labels: vec![],
            created_at: chrono::Utc::now(),
            score,
        }
    }
}
