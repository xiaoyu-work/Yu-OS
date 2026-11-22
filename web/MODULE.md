# Website Module

## Purpose

`web/` contains the static Yu OS marketing website served at the GitHub Pages
root beside the signed APT repository.

## Key Files

| Path | Role |
| --- | --- |
| `index.html` | Marketing page |
| `style.css` | Page styles |
| `app.js` | Yu Agent captions rotate automatically until a dot is selected |
| `fonts/` | Self-hosted Barlow Condensed and Space Grotesk with their SIL Open Font License texts |
| `stage.sh` | Copies the page, its fonts and `../assets/brand/` into one Pages input, stamping `@@GIT_SHA@@` so each release fetches fresh CSS and JavaScript |
| `gen-og.py` | Generates `../assets/brand/og.png` |
| `../packaging/apt-repo/build-repo.sh` | Copies the staged site into the Pages/APT artifact |

## Publication

`.github/workflows/release.yml` stages `web/` during signed OS publication.
Pages composition places the site at `/` and the APT repository under `/apt`.

## Validation

From the repository root:

```bash
bash -n web/stage.sh
bash -n packaging/apt-repo/build-repo.sh
bash web/stage.sh build/web-dist
python3 -m http.server --directory build/web-dist 8000
```

Open `http://localhost:8000/` at desktop and mobile widths; browsers block the
web fonts from `file://`. At 1200 CSS pixels and up, check that the cropped hero
art shows three slashes without overlapping the headline. Fail on broken
assets, console errors or horizontal overflow.
