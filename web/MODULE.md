# Website Module

## Purpose

`web/` contains the static Yu OS marketing website served at the GitHub Pages
root beside the signed APT repository.

## Key Files

| Path | Role |
| --- | --- |
| `index.html` | Marketing page |
| `style.css` | Page styles |
| `app.js` | Page interactions |
| `stage.sh` | Copies the page and `../assets/brand/` into one Pages input |
| `gen-og.py` | Generates `../assets/brand/og.png` |
| `../packaging/apt-repo/build-repo.sh` | Copies the staged site into the Pages/APT artifact and replaces `@@GIT_SHA@@` and `@@SUITE@@` |

## Publication

The `website` target of `.github/workflows/release.yml` stages `web/` as an
independent artifact. Pages composition places it at `/` and the APT
repository under `/apt`; neither is included in the other.

## Validation

From the repository root:

```bash
bash -n web/stage.sh
bash -n packaging/apt-repo/build-repo.sh
bash web/stage.sh build/web-dist
python3 web/gen-og.py
```

Open `build/web-dist/index.html` at desktop and mobile widths and fail on
broken assets, console errors or horizontal overflow.
