// Test-only wiring for the ZDEP-004..008 acceptance tests (kept out of
// main.rs to respect the file-structure line-count gate).

#[cfg(test)]
#[path = "../tests-rs/test_zdep_which.rs"]
mod tests_zdep_which;

#[cfg(test)]
#[path = "../tests-rs/test_zdep_glob.rs"]
mod tests_zdep_glob;

#[cfg(test)]
#[path = "../tests-rs/test_zdep_proxy_pane_errors.rs"]
mod tests_zdep_proxy_pane_errors;

#[cfg(all(test, windows))]
#[path = "../tests-rs/test_zdep_win32.rs"]
mod tests_zdep_win32;

#[cfg(test)]
#[path = "../tests-rs/test_zdep_timefmt.rs"]
mod tests_zdep_timefmt;

#[cfg(test)]
#[path = "../tests-rs/test_zdep_time_std.rs"]
mod tests_zdep_time_std;

#[cfg(test)]
#[path = "../tests-rs/test_zdep_json_types.rs"]
mod tests_zdep_json_types;
