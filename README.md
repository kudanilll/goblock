# GoBlock

A block themed casual game built with C++17, OpenGL, GLFW and GLM.

## Building on Windows

You need CMake, Ninja and a MinGW toolchain. With [scoop](https://scoop.sh) installed:

```powershell
scoop install cmake ninja mingw
```

The project uses [vcpkg](https://github.com/microsoft/vcpkg) for its libraries. Set it up once:

```powershell
git clone https://github.com/microsoft/vcpkg "$env:LOCALAPPDATA\vcpkg"
& "$env:LOCALAPPDATA\vcpkg\bootstrap-vcpkg.bat" -disableMetrics
& "$env:LOCALAPPDATA\vcpkg\vcpkg.exe" install glfw3 glew glm freeglut --triplet x64-mingw-dynamic
```

Then build:

```powershell
.\build.ps1
```

Add `-Release` for an optimized build, or `-Clean` to delete `build/` first:

```powershell
.\build.ps1 -Release
.\build.ps1 -Clean
```

If vcpkg lives somewhere else, point `VCPKG_ROOT` at it before running the script.

## Running

```powershell
.\build\bin\goblock.exe
```

The executable looks for its shaders and the font in the source tree using paths baked in at configure time, so you can launch it from any directory. The build copies the DLLs it needs (GLFW, GLEW, freeglut, and the MinGW runtime) next to `goblock.exe`.

## Building on Linux

With GLFW, GLEW, GLUT and GLM available from your package manager:

```bash
cmake -S . -B build -DCMAKE_BUILD_TYPE=Debug
cmake --build build
./build/bin/goblock
```

## License

MIT. See [LICENSE](LICENSE). Copyright (c) 2024 Achmad Daniel Syahputra.
