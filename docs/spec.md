# ytop System & Protocol Specification

## 1. Libyggterm Document Surface Protocol
* `ytop` is a document-surface app communicating with Yggterm via **OSC 7717**:
  `\e]7717;sidebar;declare;<base64-payload>\a`
* Payload registers loopback endpoint:
  * `GET /pane/topo` -> JSON array of `AppPaneWidget` for main viewport
  * `GET /pane/rail` -> JSON array of `AppPaneWidget` for sidebar rail
  * `POST /action` -> Dispatches UI commands (`mode`, `select_host`, `open_page`, `signal_pid`, `clean_jankbox`)

## 2. Shared Row Engine (`list-row`)
Every rail item is defined as:
```json
{
  "kind": "list-row",
  "id": "<unique-row-id>",
  "title": "<Row Title>",
  "subtitle": "<Descriptive Subtitle>",
  "icon": "<icon:folder | icon:archive | emoji>",
  "status": "<durable | transient | warning | danger>",
  "selected": <bool>,
  "depth": <0 | 1>,
  "expanded": <true | false | null>,
  "row_action": "<action-identifier>",
  "expand_action": "<expand-action-identifier>"
}
```

## 3. Base Notebooks Directory Structure
* Built-in base notebooks ship with the `ytop` binary.
* User & agent composed notebooks are stored on disk in:
  `~/.local/share/ytop/notebooks/` (or `~/.yggterm/notebooks/`).
