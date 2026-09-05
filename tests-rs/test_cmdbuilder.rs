use super::*;

#[test]
fn test_env() {
    // Anchor on an env var the test itself sets, rather than
    // CARGO_PKG_AUTHORS: that only held when this file compiled as part of
    // the standalone portable-pty-psmux crate, and no longer holds once
    // folded into the root crate.
    std::env::set_var("ZDEP_PTY_PROBE_VAR", "probe-value");
    let mut cmd = CommandBuilder::new("dummy");
    let probe_var = cmd.get_env("ZDEP_PTY_PROBE_VAR");
    println!("probe_var: {:?}", probe_var);
    assert!(probe_var == Some(OsStr::new("probe-value")));

    cmd.env("foo key", "foo value");
    cmd.env("bar key", "bar value");

    let iterated_envs = cmd.iter_extra_env_as_str().collect::<Vec<_>>();
    println!("iterated_envs: {:?}", iterated_envs);
    assert!(iterated_envs == vec![("bar key", "bar value"), ("foo key", "foo value")]);

    {
        let mut cmd = cmd.clone();
        cmd.env_remove("foo key");

        let iterated_envs = cmd.iter_extra_env_as_str().collect::<Vec<_>>();
        println!("iterated_envs: {:?}", iterated_envs);
        assert!(iterated_envs == vec![("bar key", "bar value")]);
    }

    {
        let mut cmd = cmd.clone();
        cmd.env_remove("bar key");

        let iterated_envs = cmd.iter_extra_env_as_str().collect::<Vec<_>>();
        println!("iterated_envs: {:?}", iterated_envs);
        assert!(iterated_envs == vec![("foo key", "foo value")]);
    }

    {
        let mut cmd = cmd.clone();
        cmd.env_clear();

        let iterated_envs = cmd.iter_extra_env_as_str().collect::<Vec<_>>();
        println!("iterated_envs: {:?}", iterated_envs);
        assert!(iterated_envs.is_empty());
    }
}

#[cfg(windows)]
#[test]
fn test_env_case_insensitive_override() {
    std::env::set_var("ZDEP_PTY_PROBE_VAR2", "probe-value-2");
    let mut cmd = CommandBuilder::new("dummy");
    cmd.env("Zdep_Pty_Probe_Var2", "Not Original");
    assert!(cmd.get_env("zdep_pty_probe_var2") == Some(OsStr::new("Not Original")));

    cmd.env_remove("zDEP_pTY_pROBE_vAR2");
    assert!(cmd.get_env("ZDEP_PTY_PROBE_VAR2").is_none());
}
