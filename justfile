build: build-ui build-core

build-ui:
  cd packages/ui && npm run build
  cp -r packages/ui/dist/* crates/ui/static/

build-core:
  cargo build

install:
  cd packages/ui && {{ if env("CI", "false") == "true" { "npm ci" } else { "npm i" } }}
