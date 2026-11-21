# License of the engine

Everything under `engine/` (the PHP engine, `engine/browser/` and its build) is licensed under the
**GNU General Public License, version 2 only** (GPL-2.0-only), because it runs and is distributed
together with IMathAS, which is GPL version 2 only:

> IMathAS (c) David Lippman, 2006-2025. This program is free software; you can redistribute it and/or
> modify it under the terms of the GNU General Public License version 2 as published by the Free
> Software Foundation. <https://github.com/drlippman/IMathAS>

The rest of D.A.D (the Rust app under `src/`) is a separate program under the MIT license (see
`LICENSE`). It talks to the engine over a request/response interface (HTTP, or `window.dadEngine`
in the browser build), which keeps the two separate works.

What this means in practice:

- Running the engine on a server is not distribution; nothing has to be published.
- The browser build (`engine/browser/dist`, deployed under the web app's `engine/`) distributes IMathAS
  and this folder to every visitor. When it is deployed for anyone outside the project, publish the
  source of `engine/` (and the IMathAS commit pinned in `engine/Dockerfile`) under GPL-2.0, for
  example as a public repository linked from the app.
- Questions are not affected. The IMathAS license states that questions written in the IMathAS
  language are not derivative works of IMathAS.
- The IMathAS author asks, as his interpretation of the GPL, that installers not charge students for
  access beyond reasonable hosting costs. D.A.D does not serve students.

Bundled third-party code keeps its own license: PHP compiled to WebAssembly (`@php-wasm/*`, GPL-2.0-or-later),
MathJax (Apache-2.0), MathQuill (MPL-2.0), jQuery (MIT).
