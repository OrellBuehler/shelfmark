# Shelfmark

A fast, modern ebook library manager built with Rust, Tauri, Svelte and shadcn-svelte.

Shelfmark opens existing calibre libraries in place. It reads and writes calibre's `metadata.db` and folder layout, so both apps can be used on the same library.

## Features

- Library with cover grid and table view, calibre search syntax, tag browser and saved searches
- Metadata editing for one or many books, embedded metadata for EPUB and PDF
- Metadata and cover download from Open Library and Google Books (set `SHELFMARK_GOOGLE_BOOKS_KEY` for the latter)
- Reader for EPUB, MOBI, AZW3, FB2, CBZ and PDF with highlights, notes and bookmarks
- Send books to Kobo, Kindle, PocketBook and Tolino readers over USB, or to any folder

Reading positions and annotations are stored in `shelfmark.db` next to `metadata.db`.

## Development

```sh
bun install
bun run tauri dev
```

```sh
bun run check
cargo test --workspace
bun run tauri build
```

`bun run dev` runs the interface in a browser against an in-memory mock library.

## Layout

- `crates/model` shared types
- `crates/library` calibre database access and search
- `crates/formats` ebook metadata, covers and online sources
- `crates/devices` e-reader detection and transfer
- `crates/reading` reading positions and annotations
- `src-tauri` desktop shell and commands
- `src` Svelte interface

## License

GPL-3.0-or-later. The library crate embeds calibre's database schema.
