# LinuxThemesStore
## _NativeWay to install linux themes on your desktop_

-  A modern desktop app for discovering, downoading Linux themes, directly from Pling & Opendesktop.
- No browser required.
- Just browse, click, and beautify your desktop!

Here are some screenshots & video preview

[![Thumbnail](https://i.ibb.co/v64sy4yh/1.png)](https://i.ibb.co/mV5vZ5Z0/1.png)[![Thumbnail](https://i.ibb.co/Xhqqdd8/2.png)](https://i.ibb.co/63kkppB/2.png)

[![Thumbnail](https://i.ibb.co/DfKX8bBj/3.png)](https://i.ibb.co/k2cn9yLP/3.png)[![Thumbnail](https://i.ibb.co/k25gTmCv/4.png)](https://i.ibb.co/s9bv4WYz/4.png)

[![Thumbnail](https://i.ibb.co/V0TK6Sgg/5.png)](https://i.ibb.co/nqjWpc77/5.png)

[![alt text](https://i.ibb.co/V0TK6Sgg/5.png)](https://github.com/debasish-patra-1987/linuxthemestore/raw/refs/heads/main/screenshots/screencastsample.mp4)

## Installation
For Arch Linux Build is available in aur..
```sh
yay -S linuxthemestore-git
```
## _Binary build_

To build this application from source, you will need the following development tools and libraries installed on your system:

* **Build System:** Meson, Ninja
* **Compilers & Toolchains:** Rust (with Cargo), C/C++ Compiler (e.g., GCC, Clang)
* **Development Libraries:**
    * GLib 2.0 (including GIO 2.0) development headers
    * GTK 3.0 development headers
* **Utilities:** pkg-config, gettext, Git

**Installation**

```sh
git clone [https://github.com/debasish-patra-1987/linuxthemestore.git](https://github.com/debasish-patra-1987/linuxthemestore.git)
cd linuxthemestore
meson setup builddir --prefix=/usr
meson compile -C builddir
sudo meson install -C builddir
```
Uninstall

```
cd linuxthemestore
cd builddir
sudo ninja uninstall

## License
MIT
**Free Software, Hell Yeah!**
