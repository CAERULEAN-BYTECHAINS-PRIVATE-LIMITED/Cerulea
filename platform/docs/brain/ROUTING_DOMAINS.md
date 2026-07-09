# Cerulea — Routing & Domain Architecture

## Domains

Main platform:
<domain>

Cerulea Studio:
studio.<domain>

## Surface boundaries

Public site:
- homepage
- marketing pages
- docs

Authenticated surfaces:
- dashboard
- project management

Studio:
- separate surface
- subdomain

## Header behavior

Public header:
- public routes only

Authenticated header:
- dashboard
- profile
- project surfaces

Studio header:
- Studio-specific controls

Header must not be static globally.
