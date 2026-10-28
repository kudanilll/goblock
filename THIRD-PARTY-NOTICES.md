# Third-party notices

## terrain-diffusion-mc

The biome rules in `crates/world/src/biome.rs` are adapted from the biome classifier in [licenses/terrain-diffusion-mc-MIT.txt](licenses/terrain-diffusion-mc-MIT.txt).

```text
The MIT License (MIT)

Copyright (c) 2025 Alexander Goslin

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
```

## Revelation

The screen-space ray march in `shaders/vulkan/water.frag` (`traceReflection`) is adapted from the screen-space ray tracing in the Revelation shader pack. GoBlock changed it: it marches a fixed number of jittered steps in uv and depth, accepts a hit only where the scene surface lies inside the last step, refines it by bisection, and runs only for water surfaces.

Revelation is licensed under the Apache License, Version 2.0. The full license text is in [licenses/Revelation-Apache-2.0.txt](licenses/Revelation-Apache-2.0.txt). You may obtain a copy of the License at <http://www.apache.org/licenses/LICENSE-2.0>. Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
