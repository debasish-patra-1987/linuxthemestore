# LinuxThemesStore
## _NativeWay to install linux themes on your desktop_

-  A modern desktop app for discovering, downloading Linux themes, directly from Pling & Opendesktop  
- No browser required.
- Just browse, click, and beautify your desktop!
- Supports GTK, GNOME Shell, icon, cursor, and KDE Plasma themes.

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
Installation
```sh
git clone https://github.com/debasish-patra-1987/linuxthemestore.git
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
