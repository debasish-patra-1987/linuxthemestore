# Linux Theme Store
**Browse, Search Categorically and manage installed themes for your Gnome/Gtk based desktop**
-  A modern desktop app for discovering, downoading Linux themes, icoirectly from Pling & Opendesktop  
- No browser required.
- Just browse, click, and beautify your desktop!

## Releases

### 1.0.5 (2025-06-18)
**Added ability to apply themes for:**
- Converted Preview Pics in Detailed Screen to Caraousel
- Bug Fixed where themes were being set randomly in Installed page

### 1.0.4 (2025-06-17)
**Added ability to apply themes for:**
- Gtk4 Themes
- Icon Themes
- Cursor Themes

### 1.0.3 (2025-06-11)
**Features:**
- Search is now separate and includes all categories
- Added Wallpapers
- Added autoscrolling when bottom of the category page is reached

### 1.0.2 (2025-06-06)
**Features:**
- Added Search functionality
- Redundant Code Cleanup


### 1.0.1 (2025-06-04)
**Performance improvements:**
- Better Image Quality for verifying themes
- Quicker Pagination and page loading
- UI Design improvements
- Redundant Code Cleanup


### 1.0.0 (2025-05-28)
**Initial Release**

---
Here are some screenshots & video preview

[![Thumbnail](https://raw.githubusercontent.com/debasish-patra-1987/linuxthemestore/ed61ee4cff31c6a936413416b94068bbf767c6fe/screenshots/1.png)](https://raw.githubusercontent.com/debasish-patra-1987/linuxthemestore/ed61ee4cff31c6a936413416b94068bbf767c6fe/screenshots/1.png)

[![Thumbnail](https://raw.githubusercontent.com/debasish-patra-1987/linuxthemestore/ed61ee4cff31c6a936413416b94068bbf767c6fe/screenshots/2.png)](https://raw.githubusercontent.com/debasish-patra-1987/linuxthemestore/ed61ee4cff31c6a936413416b94068bbf767c6fe/screenshots/1.png)

[![Thumbnail](https://raw.githubusercontent.com/debasish-patra-1987/linuxthemestore/ed61ee4cff31c6a936413416b94068bbf767c6fe/screenshots/3.png)](https://raw.githubusercontent.com/debasish-patra-1987/linuxthemestore/ed61ee4cff31c6a936413416b94068bbf767c6fe/screenshots/3.png)

[![Thumbnail](https://raw.githubusercontent.com/debasish-patra-1987/linuxthemestore/ed61ee4cff31c6a936413416b94068bbf767c6fe/screenshots/4.png)](https://raw.githubusercontent.com/debasish-patra-1987/linuxthemestore/ed61ee4cff31c6a936413416b94068bbf767c6fe/screenshots/4.png)


[![alt text](https://i.ibb.co/V0TK6Sgg/5.png)](https://github.com/debasish-patra-1987/linuxthemestore/raw/refs/heads/main/screenshots/screencastsample.mp4)


## Flatpak

[![Flathub](https://flathub.org/assets/badges/flathub-badge-en.svg)](https://flathub.org/apps/io.github.debasish_patra_1987.linuxthemestore)

## Installation

For Arch Linux Build is available in aur..
```
yay -S linuxthemestore-git
```

Manual Installation via root
```
git clone https://github.com/debasish-patra-1987/linuxthemestore.git
cd linuxthemestore
git checkout 46818ef6e0378afcd6f510a472b751a5651a989f
cargo build --release
install -Dm755 ./target/release/linuxthemestore -t /usr/bin/
install -Dm644 ./assets/io.github.debasish_patra_1987.linuxthemestore.metainfo.xml -t /usr/share/metainfo
install -Dm644 ./assets/io.github.debasish_patra_1987.linuxthemestore.svg -t /usr/share/icons/hicolor/scalable/apps
mv ./assets/io.github.debasish_patra_1987.linuxthemestore.in ./assets/io.github.debasish_patra_1987.linuxthemestore.desktop
desktop-file-install --dir=/usr/share/applications ./assets/io.github.debasish_patra_1987.linuxthemestore.desktop

```

Manual Uninstallation via root
```
rm /usr/bin/linuxthemestore
rm /usr/share/metainfo/io.github.debasish_patra_1987.linuxthemestore.metainfo.xml
rm /usr/share/icons/hicolor/scalable/apps/io.github.debasish_patra_1987.linuxthemestore.svg
rm /usr/share/applications/io.github.debasish_patra_1987.linuxthemestore.desktop
```
## License
GPL3+

If you are happy with my work, please consider donating 

[![Buy Me A Coffee](https://img.shields.io/badge/Buy%20Me%20A%20Coffee-yellow?logo=buy-me-a-coffee&logoColor=white&style=for-the-badge)](https://coff.ee/patradebasish1987)
