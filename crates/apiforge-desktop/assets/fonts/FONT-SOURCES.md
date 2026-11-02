# Bundled UI fonts

API Forge Sans is a static 400/600-weight, optical-size-14 derivative of the locally
installed Adwaita Sans (Inter). The derived family is renamed API Forge Sans.
Adwaita Mono Regular is bundled unmodified. Both ship under SIL OFL 1.1; see
LICENSE-API-Forge-fonts.txt for copyright and license text. DejaVu Sans remains
the symbol fallback. Fonts are embedded for identical metrics on Linux and Windows.

The fonts egui itself embeds (Hack, Ubuntu-Light, NotoEmoji and emoji-icon-font,
from the `epaint_default_fonts` crate) are listed with their licence texts in
`licenses/THIRD-PARTY-NOTICES.md` of each package. Liberation Sans is in this
directory but not embedded in the application.
