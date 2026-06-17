# Bundled third-party PawnIO modules

`IntelMSR.bin` and `AMDFamily17.bin` in this folder are **official, cryptographically
signed PawnIO modules** used to read CPU temperature on Windows. They are
redistributed here **unmodified**.

- **Project:** PawnIO.Modules — https://github.com/namazso/PawnIO.Modules
- **Author:** namazso
- **License:** GNU Lesser General Public License v2.1 (LGPL-2.1)
- **Driver / loader:** PawnIO — https://github.com/namazso/PawnIO • https://pawnio.eu/

Flux does **not** bundle or redistribute the PawnIO **driver** itself; the
user installs that separately (the app downloads the official signed installer
and verifies its Authenticode signature first). These `.bin` files are small
signed bytecode modules that the PawnIO driver loads and verifies; they are not
kernel drivers and contain no executable PE code.

Per LGPL-2.1, the complete corresponding source for these modules is available
at the project URL above. They are unmodified releases obtained via the same
signed set bundled by LibreHardwareMonitor.

The full LGPL-2.1 license text is available at:
https://www.gnu.org/licenses/old-licenses/lgpl-2.1.txt
