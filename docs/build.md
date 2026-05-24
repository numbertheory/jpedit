# Build Instructions

* [Install Rust](https://www.rust-lang.org/tools/install)
* Clone the repository
* If you're using nightly Rust:
  ```sh
  cargo build --release --config .cargo/release.toml
  ```
* If you're using stable Rust:
  * Ideally: Set the environment variable `RUSTC_BOOTSTRAP=1` and use the **nightly** build instructions above.
    This is recommended, because it drastically reduces the binary size and slightly improves performance.
  * Otherwise, simply run:
    ```sh
    cargo build --release
    ```

## Build Configuration

You can set the following environment variables at build-time to configure the build:

Environment variable | Description
--- | ---
`EDIT_CFG_ICU*` | See [Notes to Package Maintainers](./packaging.md) for details. Linux package maintainers are advised to review and configure these options.
`EDIT_CFG_LANGUAGES` | A comma-separated list of languages to include in the build. See [i18n/edit.toml](../i18n/edit.toml) for available languages.
