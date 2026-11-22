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
| [`../.github/workflows/publish-website.yml`](../.github/workflows/publish-website.yml) | Publishes the static site without building OS packages |

## Publication

Run **Publish Website** on `main` to publish `web/`. It does not build Yu OS,
contact the upstream App APT repository, or use signing secrets.

Both publications share one Pages site, so they run under the same deployment
lock. The website runs at `/` and retains the signed files under `/apt`
byte-for-byte. **Release Yu OS** updates `/apt` and retains the live website.
Before a signed OS archive is live, run **Publish Website** with `allow_no_apt`
enabled. That input requires both the current and legacy archive paths to be
absent; it does not bypass missing or invalid files in an existing archive.
Publish the website once with this workflow to establish its file inventory
before using the split OS release workflow.

## Validation

From the repository root:

```bash
bash -n web/stage.sh
bash web/stage.sh build/web-dist
python3 -m http.server --directory build/web-dist 8000
```

Open `http://localhost:8000/` at desktop and mobile widths; browsers block the
web fonts from `file://`. At 1200 CSS pixels and up, check that the cropped hero
art shows three slashes without overlapping the headline. Fail on broken
assets, console errors or horizontal overflow.
