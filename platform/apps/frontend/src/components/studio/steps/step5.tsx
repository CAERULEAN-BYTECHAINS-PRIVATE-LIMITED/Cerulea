"use client";

import React, { useEffect, useMemo, useState, useCallback } from "react";
import StepGuidance from '@/components/studio/StepGuidance';
import {
  Alert,
  Box,
  Button,
  Chip,
  Divider,
  IconButton,
  MenuItem,
  Paper,
  Snackbar,
  Stack,
  Tabs,
  Tab,
  TextField,
  Tooltip,
  Typography,
  FormControl,
  InputLabel,
  Select,
  Switch,
  FormControlLabel,
} from "@mui/material";
import ArrowBackIcon from "@mui/icons-material/ArrowBack";
import SaveIcon from "@mui/icons-material/Save";
import SearchIcon from "@mui/icons-material/Search";
import DeleteIcon from "@mui/icons-material/Delete";
import AddIcon from "@mui/icons-material/Add";

import { DragDropContext, Droppable, Draggable, DropResult } from "@hello-pangea/dnd";

import GridLayout, { Layout } from "react-grid-layout";
import { WidthProvider } from "react-grid-layout";
import "react-grid-layout/css/styles.css";
import "react-resizable/css/styles.css";

const RGL = WidthProvider(GridLayout);

type ProjectType = "dapp" | "blockchain";
type AnyObj = Record<string, any>;

type UiElementType =
  | "Heading"
  | "Text"
  | "Button"
  | "TextField"
  | "Textarea"
  | "Checkbox"
  | "Switch"
  | "Select"
  | "Card"
  | "Divider"
  | "Chip"
  | "Alert"
  | "Stat"
  | "Table"
  | "Form"
  | "LoginBox"
  | "Hero"
  | "Navbar"
  | "Footer"
  | "PricingCard"
  | "FeatureList";

type UiElement = {
  id: string;
  type: UiElementType;
  props: AnyObj;
};

type UiBinding = {
  kind: "text" | "value" | "action" | "list";
  ref: string;
};

type UiState = {
  pages: { id: string; name: string }[];
  activePageId: string;
  elementsByPage: Record<string, UiElement[]>;
  layoutByPage: Record<string, Layout[]>;
  bindings: Record<string, UiBinding | undefined>;
};

function uid(prefix = "id") {
  return `${prefix}_${Math.random().toString(36).slice(2, 10)}${Date.now().toString(36)}`;
}

function safeJsonParse<T>(v: string, fallback: T): T {
  try {
    return JSON.parse(v) as T;
  } catch {
    return fallback;
  }
}

function countEntities(schema: any): { entities: string[]; fieldsByEntity: Record<string, string[]> } {
  const entities: string[] = [];
  const fieldsByEntity: Record<string, string[]> = {};

  const rawEntities = schema?.entities ?? schema?.tables ?? schema?.models ?? schema?.schema ?? null;

  if (Array.isArray(rawEntities)) {
    for (const e of rawEntities) {
      const name = String(e?.name ?? e?.title ?? e?.id ?? "Entity");
      entities.push(name);
      const f = Array.isArray(e?.fields) ? e.fields : Array.isArray(e?.columns) ? e.columns : [];
      fieldsByEntity[name] = f.map((x: any) => String(x?.name ?? x?.key ?? x?.id ?? "field"));
    }
  } else if (Array.isArray(schema)) {
    for (const e of schema) {
      const name = String(e?.name ?? e?.id ?? "Entity");
      entities.push(name);
      const f = Array.isArray(e?.fields) ? e.fields : [];
      fieldsByEntity[name] = f.map((x: any) => String(x?.name ?? x?.id ?? "field"));
    }
  }

  return { entities, fieldsByEntity };
}

type PaletteKind = "component" | "template";

type PaletteItem = {
  id: string;
  kind: PaletteKind;
  label: string;
  category: "Components" | "Templates";
  type?: UiElementType; // only for component
  preview?: AnyObj; // props used for preview + initial props
  templateId?: string; // only for template
};

function buildPalette(track: ProjectType): PaletteItem[] {
  const commonComponents: PaletteItem[] = [
    { id: "c_heading", kind: "component", label: "Heading", category: "Components", type: "Heading", preview: { text: "Heading", variant: "h5", align: "left" } },
    { id: "c_text", kind: "component", label: "Text", category: "Components", type: "Text", preview: { text: "Body text", variant: "body2", align: "left" } },
    { id: "c_button", kind: "component", label: "Button", category: "Components", type: "Button", preview: { label: "Button", variant: "contained", size: "medium", fullWidth: false } },
    { id: "c_textfield", kind: "component", label: "Text Field", category: "Components", type: "TextField", preview: { label: "Field", placeholder: "", size: "medium" } },
    { id: "c_textarea", kind: "component", label: "Textarea", category: "Components", type: "Textarea", preview: { label: "Textarea", placeholder: "", rows: 4 } },
    { id: "c_switch", kind: "component", label: "Switch", category: "Components", type: "Switch", preview: { label: "Switch", checked: false } },
    { id: "c_select", kind: "component", label: "Select", category: "Components", type: "Select", preview: { label: "Select", options: ["Option A", "Option B"], value: "Option A" } },
    { id: "c_card", kind: "component", label: "Card", category: "Components", type: "Card", preview: { title: "Card Title", subtitle: "Card subtitle", body: "Card content" } },
    { id: "c_divider", kind: "component", label: "Divider", category: "Components", type: "Divider", preview: {} },
    { id: "c_chip", kind: "component", label: "Chip", category: "Components", type: "Chip", preview: { label: "Chip" } },
    { id: "c_alert", kind: "component", label: "Alert", category: "Components", type: "Alert", preview: { severity: "info", text: "Alert message" } },
    { id: "c_stat", kind: "component", label: "Stat", category: "Components", type: "Stat", preview: { label: "Metric", value: "N/A", helper: "Click to bind" } },
    { id: "c_table", kind: "component", label: "Table", category: "Components", type: "Table", preview: { title: "Table", columns: ["id", "name", "status"], rows: 5 } },
    { id: "c_form", kind: "component", label: "Form", category: "Components", type: "Form", preview: { title: "Create Record", fields: ["name", "email"], submitLabel: "Submit" } },
  ];

  const dappOnly: PaletteItem[] = [
    { id: "c_login", kind: "component", label: "Login Box", category: "Components", type: "LoginBox", preview: { title: "Sign in", subtitle: "Access your app" } },
    { id: "c_hero", kind: "component", label: "Hero", category: "Components", type: "Hero", preview: { title: "Your headline", subtitle: "A clear value prop", cta: "Get Started" } },
    { id: "c_navbar", kind: "component", label: "Navbar", category: "Components", type: "Navbar", preview: { brand: "App", links: ["Home", "Pricing", "Docs"], cta: "Sign In" } },
    { id: "c_footer", kind: "component", label: "Footer", category: "Components", type: "Footer", preview: { text: "© Your Company" } },
    { id: "c_pricing", kind: "component", label: "Pricing Card", category: "Components", type: "PricingCard", preview: { plan: "Starter", price: "₹0", features: ["Feature A", "Feature B"], cta: "Choose" } },
    { id: "c_features", kind: "component", label: "Feature List", category: "Components", type: "FeatureList", preview: { title: "Features", items: ["Fast", "Secure", "Customizable"] } },
  ];

  // IMPORTANT: Templates are the “Presets” you wanted but fully working.
  const commonTemplates: PaletteItem[] = [
    { id: "t_crud_list", kind: "template", label: "Template: List + Details", category: "Templates", templateId: "crud_list_details" },
    { id: "t_crud_form", kind: "template", label: "Template: Create/Edit Form", category: "Templates", templateId: "crud_form" },
  ];

  const dappTemplates: PaletteItem[] = [
    { id: "t_landing", kind: "template", label: "Template: Landing Page", category: "Templates", templateId: "dapp_landing" },
    { id: "t_auth", kind: "template", label: "Template: Login Page", category: "Templates", templateId: "dapp_login" },
    { id: "t_pricing", kind: "template", label: "Template: Pricing Page", category: "Templates", templateId: "dapp_pricing" },
  ];

  const chainTemplates: PaletteItem[] = [
    { id: "t_chain_dash", kind: "template", label: "Template: Chain Dashboard", category: "Templates", templateId: "chain_dashboard" },
    { id: "t_validators", kind: "template", label: "Template: Validators Table", category: "Templates", templateId: "chain_validators" },
  ];

  const components = track === "dapp" ? [...commonComponents, ...dappOnly] : [...commonComponents];
  const templates = track === "dapp" ? [...commonTemplates, ...dappTemplates] : [...commonTemplates, ...chainTemplates];

  return [...templates, ...components];
}

function renderElement(el: UiElement, binding?: UiBinding) {
  const p = el.props ?? {};
  const boundHint = binding ? ` (bound: ${binding.ref})` : "";

  switch (el.type) {
    case "Heading":
      return (
        <Typography variant={p.variant ?? "h5"} sx={{ fontWeight: 700 }} align={p.align ?? "left"}>
          {String(p.text ?? "Heading")}
          {binding ? (
            <Typography component="span" variant="caption" sx={{ opacity: 0.6, ml: 1 }}>
              {boundHint}
            </Typography>
          ) : null}
        </Typography>
      );
    case "Text":
      return (
        <Typography variant={p.variant ?? "body2"} align={p.align ?? "left"}>
          {String(p.text ?? "Text")}
          {binding ? (
            <Typography component="span" variant="caption" sx={{ opacity: 0.6, ml: 1 }}>
              {boundHint}
            </Typography>
          ) : null}
        </Typography>
      );
    case "Button":
      return (
        <Button variant={p.variant ?? "contained"} size={p.size ?? "medium"} fullWidth={!!p.fullWidth}>
          {String(p.label ?? "Button")}
        </Button>
      );
    case "TextField":
      return <TextField label={p.label ?? "Field"} placeholder={p.placeholder ?? ""} size={p.size ?? "medium"} fullWidth />;
    case "Textarea":
      return <TextField label={p.label ?? "Textarea"} placeholder={p.placeholder ?? ""} fullWidth multiline rows={Number(p.rows ?? 4)} />;
    case "Checkbox":
      // Keep checkbox as switch-style placeholder (fine for builder)
      return <FormControlLabel control={<Switch checked={!!p.checked} />} label={p.label ?? "Checkbox"} />;
    case "Switch":
      return <FormControlLabel control={<Switch checked={!!p.checked} />} label={p.label ?? "Switch"} />;
    case "Select":
      return (
        <FormControl fullWidth>
          <InputLabel>{p.label ?? "Select"}</InputLabel>
          <Select label={p.label ?? "Select"} value={p.value ?? ""} onChange={() => {}}>
            {(Array.isArray(p.options) ? p.options : []).map((o: any, i: number) => (
              <MenuItem key={i} value={String(o)}>
                {String(o)}
              </MenuItem>
            ))}
          </Select>
        </FormControl>
      );
    case "Card":
      return (
        <Paper sx={{ p: 2, borderRadius: 3, border: "1px solid rgba(255,255,255,0.10)", background: "rgba(255,255,255,0.03)" }}>
          <Typography sx={{ fontWeight: 700, pl: 1.25 }}>{p.title ?? "Card Title"}</Typography>
          <Typography variant="body2" sx={{ opacity: 0.75, mb: 1 }}>
            {p.subtitle ?? "Card subtitle"}
          </Typography>
          <Typography variant="body2">{p.body ?? "Card body content"}</Typography>
        </Paper>
      );
    case "Divider":
      return <Divider />;
    case "Chip":
      return <Chip label={p.label ?? "Chip"} />;
    case "Alert":
      return <Alert severity={p.severity ?? "info"}>{p.text ?? "Alert message"}</Alert>;
    case "Stat":
      return (
        <Paper sx={{ p: 2, borderRadius: 3, border: "1px solid rgba(255,255,255,0.10)", background: "rgba(255,255,255,0.03)" }}>
          <Typography variant="body2" sx={{ opacity: 0.75 }}>
            {p.label ?? "Stat"}
          </Typography>
          <Typography variant="h6" sx={{ fontWeight: 800 }}>
            {p.value ?? "N/A"}
          </Typography>
          <Typography variant="caption" sx={{ opacity: 0.7 }}>
            {p.helper ?? ""}
          </Typography>
        </Paper>
      );
    case "Table": {
      const cols = Array.isArray(p.columns) ? p.columns : ["id", "name"];
      const rows = Number(p.rows ?? 5);
      return (
        <Paper sx={{ p: 2, borderRadius: 3, border: "1px solid rgba(255,255,255,0.10)", background: "rgba(255,255,255,0.03)" }}>
          <Stack direction="row" alignItems="center" justifyContent="space-between" sx={{ mb: 1 }}>
            <Typography sx={{ fontWeight: 700, pl: 1.25 }}>{p.title ?? "Table"}</Typography>
            {binding ? <Chip size="small" label={`list: ${binding.ref}`} /> : <Chip size="small" label="not bound" variant="outlined" />}
          </Stack>
          <Box sx={{ overflowX: "auto" }}>
            <Box component="table" sx={{ width: "100%", borderCollapse: "collapse" }}>
              <Box component="thead">
                <Box component="tr">
                  {cols.map((c: any, i: number) => (
                    <Box
                      key={i}
                      component="th"
                      sx={{
                        textAlign: "left",
                        fontSize: 12,
                        padding: "8px",
                        borderBottom: "1px solid rgba(255,255,255,0.12)",
                        opacity: 0.8,
                      }}
                    >
                      {String(c)}
                    </Box>
                  ))}
                </Box>
              </Box>
              <Box component="tbody">
                {Array.from({ length: rows }).map((_, r) => (
                  <Box component="tr" key={r}>
                    {cols.map((_: any, i: number) => (
                      <Box key={i} component="td" sx={{ padding: "8px", fontSize: 12, borderBottom: "1px solid rgba(255,255,255,0.06)", opacity: 0.8 }}>
                        —
                      </Box>
                    ))}
                  </Box>
                ))}
              </Box>
            </Box>
          </Box>
        </Paper>
      );
    }
    case "Form": {
      const fields = Array.isArray(p.fields) ? p.fields : ["name"];
      return (
        <Paper sx={{ p: 2, borderRadius: 3, border: "1px solid rgba(255,255,255,0.10)", background: "rgba(255,255,255,0.03)" }}>
          <Typography sx={{ fontWeight: 700, mb: 1 }}>{p.title ?? "Form"}</Typography>
          <Stack spacing={1.2}>
            {fields.map((f: any, i: number) => (
              <TextField key={i} label={String(f)} size="small" fullWidth />
            ))}
            <Button variant="contained">{p.submitLabel ?? "Submit"}</Button>
          </Stack>
        </Paper>
      );
    }
    case "LoginBox":
      return (
        <Paper sx={{ p: 2, borderRadius: 3, border: "1px solid rgba(255,255,255,0.10)", background: "rgba(255,255,255,0.03)" }}>
          <Typography sx={{ fontWeight: 800, mb: 0.5 }}>{p.title ?? "Sign in"}</Typography>
          <Typography variant="body2" sx={{ opacity: 0.75, mb: 1.5 }}>
            {p.subtitle ?? "Access your app"}
          </Typography>
          <Stack spacing={1}>
            <TextField label="Email" size="small" fullWidth />
            <TextField label="Password" size="small" type="password" fullWidth />
            <Button variant="contained">Sign in</Button>
          </Stack>
        </Paper>
      );
    case "Hero":
      return (
        <Paper sx={{ p: 2.5, borderRadius: 3, border: "1px solid rgba(255,255,255,0.10)", background: "rgba(255,255,255,0.03)" }}>
          <Typography variant="h5" sx={{ fontWeight: 900 }}>
            {p.title ?? "Your headline"}
          </Typography>
          <Typography variant="body2" sx={{ opacity: 0.75, my: 1 }}>
            {p.subtitle ?? "A clear value prop"}
          </Typography>
          <Button variant="contained">{p.cta ?? "Get Started"}</Button>
        </Paper>
      );
    case "Navbar":
      return (
        <Paper sx={{ p: 1.5, borderRadius: 3, border: "1px solid rgba(255,255,255,0.10)", background: "rgba(255,255,255,0.03)" }}>
          <Stack direction="row" alignItems="center" justifyContent="space-between">
            <Typography sx={{ fontWeight: 900, pl: 1.25 }}>{p.brand ?? "App"}</Typography>
            <Stack direction="row" spacing={1} alignItems="center">
              {(Array.isArray(p.links) ? p.links : []).slice(0, 4).map((x: any, i: number) => (
                <Typography key={i} variant="body2" sx={{ opacity: 0.8 }}>
                  {String(x)}
                </Typography>
              ))}
              <Button size="small" variant="outlined">
                {p.cta ?? "Sign In"}
              </Button>
            </Stack>
          </Stack>
        </Paper>
      );
    case "Footer":
      return (
        <Paper sx={{ p: 1.5, borderRadius: 3, border: "1px solid rgba(255,255,255,0.10)", background: "rgba(255,255,255,0.03)" }}>
          <Typography variant="body2" sx={{ opacity: 0.75 }}>
            {p.text ?? "© Your Company"}
          </Typography>
        </Paper>
      );
    case "PricingCard":
      return (
        <Paper sx={{ p: 2, borderRadius: 3, border: "1px solid rgba(255,255,255,0.10)", background: "rgba(255,255,255,0.03)" }}>
          <Typography sx={{ fontWeight: 800, pl: 1.25 }}>{p.plan ?? "Starter"}</Typography>
          <Typography variant="h6" sx={{ fontWeight: 900, my: 1 }}>
            {p.price ?? "₹0"}
          </Typography>
          <Stack spacing={0.5} sx={{ mb: 1 }}>
            {(Array.isArray(p.features) ? p.features : []).slice(0, 6).map((f: any, i: number) => (
              <Typography key={i} variant="body2" sx={{ opacity: 0.8 }}>
                • {String(f)}
              </Typography>
            ))}
          </Stack>
          <Button variant="contained" fullWidth>
            {p.cta ?? "Choose"}
          </Button>
        </Paper>
      );
    case "FeatureList":
      return (
        <Paper sx={{ p: 2, borderRadius: 3, border: "1px solid rgba(255,255,255,0.10)", background: "rgba(255,255,255,0.03)" }}>
          <Typography sx={{ fontWeight: 900, mb: 1 }}>{p.title ?? "Features"}</Typography>
          <Stack spacing={0.5}>
            {(Array.isArray(p.items) ? p.items : []).slice(0, 10).map((f: any, i: number) => (
              <Typography key={i} variant="body2" sx={{ opacity: 0.8 }}>
                • {String(f)}
              </Typography>
            ))}
          </Stack>
        </Paper>
      );
    default:
      return <Typography>Unknown component</Typography>;
  }
}

export default function Step5({
  goPrev,
  goNext,
  projectId,
}: {
  goPrev?: () => void;
  goNext?: () => void;
  projectId: string | null;
}) {
  const [project, setProject] = useState<any | null>(null);
  const [projectType, setProjectType] = useState<ProjectType>("dapp");

  const [toast, setToast] = useState<{ open: boolean; msg: string; kind: "success" | "error" | "info" }>({
    open: false,
    msg: "",
    kind: "info",
  });

  // Track-aware palette (Fix #8)
  const palette = useMemo(() => buildPalette(projectType), [projectType]);
  const [paletteQuery, setPaletteQuery] = useState("");
  const [paletteTab, setPaletteTab] = useState<"All" | "Templates" | "Components">("All");

  const [ui, setUi] = useState<UiState>(() => {
    const homeId = "page_home";
    return {
      pages: [{ id: homeId, name: "Home" }],
      activePageId: homeId,
      elementsByPage: { [homeId]: [] },
      layoutByPage: { [homeId]: [] },
      bindings: {},
    };
  });

  const activeElements = ui.elementsByPage[ui.activePageId] ?? [];
  const activeLayout = ui.layoutByPage[ui.activePageId] ?? [];

  const [selectedId, setSelectedId] = useState<string | null>(null);

  const schemaInfo = useMemo(() => {
    const schema = project?.schema ?? null;
    return countEntities(schema);
  }, [project]);

  const allBindingOptions = useMemo(() => {
    const out: { label: string; value: string }[] = [];
    for (const e of schemaInfo.entities) {
      const fields = schemaInfo.fieldsByEntity[e] ?? [];
      for (const f of fields) out.push({ label: `${e}.${f}`, value: `${e}.${f}` });
      out.push({ label: `${e}[]`, value: `${e}[]` });
    }
    if (!out.length) out.push({ label: "No schema found", value: "" });
    return out;
  }, [schemaInfo]);

  // Track fallback from Step 0 localStorage (Fix #8)
  useEffect(() => {
    const q =
      typeof window !== "undefined"
        ? ((window.localStorage.getItem("cerulea.projectType") ||
            window.localStorage.getItem("cbc.projectType")) as ProjectType | null)
        : null;

    if (q === "dapp" || q === "blockchain") setProjectType(q);
  }, []);

  // Load project + hydrate UI from DB (also sets track if project has it)
  useEffect(() => {
    let alive = true;
    async function run() {
      if (!projectId) {
        setProject(null);
        // Fix #7: Remove "no project loaded" banner (we simply don’t show anything here)
        return;
      }
      try {
        const r = await fetch(`/api/projects/${projectId}`, { cache: "no-store" });
        if (!r.ok) throw new Error(`GET project failed: ${r.status}`);
        const data = await r.json();
        if (!alive) return;

        setProject(data);

        const pt = (data?.projectType as ProjectType | undefined) ?? projectType;
        if (pt === "dapp" || pt === "blockchain") setProjectType(pt);

        const stored = data?.ui ?? null;
        if (stored?.pages && stored?.activePageId && stored?.elementsByPage && stored?.layoutByPage) {
          setUi((prev) => ({ ...prev, ...stored }));
        }
      } catch (e: any) {
        setToast({ open: true, msg: e?.message ?? "Failed to load project", kind: "error" });
      }
    }
    run();
    return () => {
      alive = false;
    };
  }, [projectId]); // intentionally not depending on projectType

  const filteredPalette = useMemo(() => {
    const q = paletteQuery.trim().toLowerCase();
    return palette.filter((it) => {
      const matchTab =
        paletteTab === "All" ? true : paletteTab === "Templates" ? it.category === "Templates" : it.category === "Components";
      const matchQ = !q ? true : `${it.label} ${it.category}`.toLowerCase().includes(q);
      return matchTab && matchQ;
    });
  }, [palette, paletteQuery, paletteTab]);

  const createElementFromPalette = useCallback((item: PaletteItem): UiElement => {
    const baseProps = item.preview ?? {};
    return { id: uid("ui"), type: item.type as UiElementType, props: { ...baseProps } };
  }, []);

  // Templates: when dropped, they add MULTIPLE items + layout (Fix #6)
  const applyTemplate = useCallback(
    (templateId: string) => {
      const make = (type: UiElementType, props: AnyObj) => ({ id: uid("ui"), type, props });

      if (templateId === "crud_list_details") {
        const h = make("Heading", { text: "Records", variant: "h5" });
        const t = make("Table", { title: "Records", columns: ["id", "name", "status"], rows: 8 });
        const d = make("Card", { title: "Details", subtitle: "Select a row", body: "Details will appear here." });
        const b = make("Button", { label: "Create New", variant: "contained", size: "medium" });

        return {
          elements: [h, b, t, d],
          layout: [
            { i: h.id, x: 0, y: Infinity, w: 6, h: 3, minW: 2, minH: 2 },
            { i: b.id, x: 6, y: Infinity, w: 3, h: 3, minW: 2, minH: 2 },
            { i: t.id, x: 0, y: Infinity, w: 8, h: 12, minW: 4, minH: 6 },
            { i: d.id, x: 8, y: Infinity, w: 4, h: 12, minW: 3, minH: 6 },
          ],
        };
      }

      if (templateId === "crud_form") {
        const h = make("Heading", { text: "Create / Edit", variant: "h5" });
        const f = make("Form", { title: "Record", fields: ["name", "email", "status"], submitLabel: "Save" });
        const a = make("Alert", { severity: "info", text: "Tip: bind fields to schema on the right." });
        return {
          elements: [h, a, f],
          layout: [
            { i: h.id, x: 0, y: Infinity, w: 12, h: 3, minW: 2, minH: 2 },
            { i: a.id, x: 0, y: Infinity, w: 12, h: 3, minW: 2, minH: 2 },
            { i: f.id, x: 0, y: Infinity, w: 6, h: 12, minW: 4, minH: 8 },
          ],
        };
      }

      if (templateId === "dapp_landing") {
        const nav = make("Navbar", { brand: "Your App", links: ["Home", "Pricing", "Docs"], cta: "Sign In" });
        const hero = make("Hero", { title: "Build your dApp faster", subtitle: "Drag, bind, deploy.", cta: "Get Started" });
        const feat = make("FeatureList", { title: "Why it works", items: ["Fast", "Secure", "Composable", "Beautiful UI"] });
        const price = make("PricingCard", { plan: "Starter", price: "₹0", features: ["1 project", "Basic analytics", "Email support"], cta: "Choose" });
        const foot = make("Footer", { text: "© Caerulean Bytechains Pvt Ltd" });
        return {
          elements: [nav, hero, feat, price, foot],
          layout: [
            { i: nav.id, x: 0, y: Infinity, w: 12, h: 3, minW: 6, minH: 2 },
            { i: hero.id, x: 0, y: Infinity, w: 12, h: 8, minW: 6, minH: 6 },
            { i: feat.id, x: 0, y: Infinity, w: 6, h: 8, minW: 4, minH: 6 },
            { i: price.id, x: 6, y: Infinity, w: 6, h: 8, minW: 4, minH: 6 },
            { i: foot.id, x: 0, y: Infinity, w: 12, h: 3, minW: 6, minH: 2 },
          ],
        };
      }

      if (templateId === "dapp_login") {
        const h = make("Heading", { text: "Welcome back", variant: "h5" });
        const login = make("LoginBox", { title: "Sign in", subtitle: "Access your account" });
        return {
          elements: [h, login],
          layout: [
            { i: h.id, x: 0, y: Infinity, w: 12, h: 3, minW: 2, minH: 2 },
            { i: login.id, x: 3, y: Infinity, w: 6, h: 12, minW: 4, minH: 8 },
          ],
        };
      }

      if (templateId === "dapp_pricing") {
        const h = make("Heading", { text: "Pricing", variant: "h5" });
        const p1 = make("PricingCard", { plan: "Starter", price: "₹0", features: ["Core builder", "1 project"], cta: "Choose" });
        const p2 = make("PricingCard", { plan: "Pro", price: "₹999/mo", features: ["Unlimited projects", "Team access"], cta: "Upgrade" });
        const p3 = make("PricingCard", { plan: "Enterprise", price: "Talk to us", features: ["SSO", "Support", "Compliance"], cta: "Contact" });
        return {
          elements: [h, p1, p2, p3],
          layout: [
            { i: h.id, x: 0, y: Infinity, w: 12, h: 3, minW: 2, minH: 2 },
            { i: p1.id, x: 0, y: Infinity, w: 4, h: 10, minW: 3, minH: 6 },
            { i: p2.id, x: 4, y: Infinity, w: 4, h: 10, minW: 3, minH: 6 },
            { i: p3.id, x: 8, y: Infinity, w: 4, h: 10, minW: 3, minH: 6 },
          ],
        };
      }

      if (templateId === "chain_dashboard") {
        const h = make("Heading", { text: "Network Dashboard", variant: "h5" });
        const s1 = make("Stat", { label: "Blocks", value: "N/A", helper: "bind to runtime" });
        const s2 = make("Stat", { label: "TPS", value: "N/A", helper: "bind to runtime" });
        const s3 = make("Stat", { label: "Validators", value: "N/A", helper: "bind to runtime" });
        const t = make("Table", { title: "Recent Activity", columns: ["time", "type", "details"], rows: 8 });
        return {
          elements: [h, s1, s2, s3, t],
          layout: [
            { i: h.id, x: 0, y: Infinity, w: 12, h: 3, minW: 2, minH: 2 },
            { i: s1.id, x: 0, y: Infinity, w: 4, h: 6, minW: 3, minH: 4 },
            { i: s2.id, x: 4, y: Infinity, w: 4, h: 6, minW: 3, minH: 4 },
            { i: s3.id, x: 8, y: Infinity, w: 4, h: 6, minW: 3, minH: 4 },
            { i: t.id, x: 0, y: Infinity, w: 12, h: 12, minW: 6, minH: 8 },
          ],
        };
      }

      if (templateId === "chain_validators") {
        const h = make("Heading", { text: "Validators", variant: "h5" });
        const t = make("Table", { title: "Validators", columns: ["id", "uptime", "score", "status"], rows: 10 });
        return {
          elements: [h, t],
          layout: [
            { i: h.id, x: 0, y: Infinity, w: 12, h: 3, minW: 2, minH: 2 },
            { i: t.id, x: 0, y: Infinity, w: 12, h: 14, minW: 6, minH: 10 },
          ],
        };
      }

      // Fallback: do nothing
      return { elements: [], layout: [] };
    },
    []
  );

  const addPage = useCallback(() => {
    const id = uid("page");
    const name = `Page ${ui.pages.length + 1}`;
    setUi((prev) => ({
      ...prev,
      pages: [...prev.pages, { id, name }],
      activePageId: id,
      elementsByPage: { ...prev.elementsByPage, [id]: [] },
      layoutByPage: { ...prev.layoutByPage, [id]: [] },
    }));
    setSelectedId(null);
  }, [ui.pages.length]);

  const renameActivePage = useCallback((name: string) => {
    setUi((prev) => ({
      ...prev,
      pages: prev.pages.map((p) => (p.id === prev.activePageId ? { ...p, name } : p)),
    }));
  }, []);

  const deleteActivePage = useCallback(() => {
    setUi((prev) => {
      if (prev.pages.length <= 1) return prev;
      const nextPages = prev.pages.filter((p) => p.id !== prev.activePageId);
      const nextActive = nextPages[0]?.id ?? prev.activePageId;

      const nextElementsByPage = { ...prev.elementsByPage };
      const nextLayoutByPage = { ...prev.layoutByPage };
      delete nextElementsByPage[prev.activePageId];
      delete nextLayoutByPage[prev.activePageId];

      return {
        ...prev,
        pages: nextPages,
        activePageId: nextActive,
        elementsByPage: nextElementsByPage,
        layoutByPage: nextLayoutByPage,
      };
    });
    setSelectedId(null);
  }, []);

  const onDragEnd = useCallback(
    (result: DropResult) => {
      const { source, destination, draggableId } = result;
      if (!destination) return;

      // palette -> canvas
      if (source.droppableId === "palette" && destination.droppableId === "canvas") {
        const item = filteredPalette.find((x) => x.id === draggableId) ?? palette.find((x) => x.id === draggableId);
        if (!item) return;

        // Template drop (Fix #6)
        if (item.kind === "template" && item.templateId) {
          const tpl = applyTemplate(item.templateId);
          if (!tpl.elements.length) return;

          setUi((prev) => {
            const pageId = prev.activePageId;
            const els = prev.elementsByPage[pageId] ?? [];
            const layout = prev.layoutByPage[pageId] ?? [];
            return {
              ...prev,
              elementsByPage: { ...prev.elementsByPage, [pageId]: [...els, ...tpl.elements] },
              layoutByPage: { ...prev.layoutByPage, [pageId]: [...layout, ...tpl.layout] },
            };
          });

          setSelectedId(tpl.elements[0]?.id ?? null);
          return;
        }

        // Component drop
        if (item.kind === "component" && item.type) {
          const el = createElementFromPalette(item);

          setUi((prev) => {
            const pageId = prev.activePageId;
            const els = prev.elementsByPage[pageId] ?? [];
            const layout = prev.layoutByPage[pageId] ?? [];

            const nextEls = [...els, el];
            const n = nextEls.length;

            const nextLayout: Layout[] = [
              ...layout,
              {
                i: el.id,
                x: (n * 2) % 12,
                y: Infinity,
                w: item.type === "Hero" || item.type === "Navbar" || item.type === "Footer" ? 12 : item.type === "Table" ? 8 : 4,
                h: item.type === "Hero" ? 8 : item.type === "Table" ? 10 : item.type === "Form" ? 10 : 4,
                minW: 2,
                minH: 2,
              },
            ];

            return {
              ...prev,
              elementsByPage: { ...prev.elementsByPage, [pageId]: nextEls },
              layoutByPage: { ...prev.layoutByPage, [pageId]: nextLayout },
            };
          });

          setSelectedId(el.id);
          return;
        }
      }
    },
    [filteredPalette, palette, createElementFromPalette, applyTemplate]
  );

  const onLayoutChange = useCallback((next: Layout[]) => {
    setUi((prev) => ({
      ...prev,
      layoutByPage: { ...prev.layoutByPage, [prev.activePageId]: next },
    }));
  }, []);

  const selectedElement = useMemo(() => activeElements.find((e) => e.id === selectedId) ?? null, [activeElements, selectedId]);
  const selectedBinding = useMemo(() => (selectedId ? ui.bindings[selectedId] : undefined), [selectedId, ui.bindings]);

  const updateSelectedProps = useCallback(
    (patch: AnyObj) => {
      if (!selectedId) return;
      setUi((prev) => {
        const pageId = prev.activePageId;
        const els = prev.elementsByPage[pageId] ?? [];
        const nextEls = els.map((e) => (e.id === selectedId ? { ...e, props: { ...(e.props ?? {}), ...patch } } : e));
        return { ...prev, elementsByPage: { ...prev.elementsByPage, [pageId]: nextEls } };
      });
    },
    [selectedId]
  );

  const setBinding = useCallback(
    (b: UiBinding | undefined) => {
      if (!selectedId) return;
      setUi((prev) => ({
        ...prev,
        bindings: { ...prev.bindings, [selectedId]: b },
      }));
    },
    [selectedId]
  );

  const deleteSelected = useCallback(() => {
    if (!selectedId) return;
    setUi((prev) => {
      const pageId = prev.activePageId;
      const els = prev.elementsByPage[pageId] ?? [];
      const layout = prev.layoutByPage[pageId] ?? [];
      const nextEls = els.filter((e) => e.id !== selectedId);
      const nextLayout = layout.filter((l) => l.i !== selectedId);

      const nextBindings = { ...prev.bindings };
      delete nextBindings[selectedId];

      return {
        ...prev,
        elementsByPage: { ...prev.elementsByPage, [pageId]: nextEls },
        layoutByPage: { ...prev.layoutByPage, [pageId]: nextLayout },
        bindings: nextBindings,
      };
    });
    setSelectedId(null);
  }, [selectedId]);

  const saveUi = useCallback(async () => {
    if (!projectId) {
      setToast({ open: true, msg: "No projectId found. Open the studio with ?projectId=...", kind: "error" });
      return;
    }
    try {
      const r = await fetch(`/api/projects/${projectId}/ui`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ ui }),
      });
      if (!r.ok) throw new Error(`Save failed: ${r.status}`);
      setToast({ open: true, msg: "UI saved", kind: "success" });
    } catch (e: any) {
      setToast({ open: true, msg: e?.message ?? "Save failed", kind: "error" });
    }
  }, [projectId, ui]);

  const handleNext = useCallback(() => {
    if (goNext) goNext();
  }, [goNext]);

  const handleBack = useCallback(() => {
    if (goPrev) goPrev();
  }, [goPrev]);

  const pageName = useMemo(() => ui.pages.find((p) => p.id === ui.activePageId)?.name ?? "Page", [ui.pages, ui.activePageId]);

  // Inspector fields
  const [sxDraft, setSxDraft] = useState<string>("{}");
  useEffect(() => {
    if (!selectedElement) return;
    setSxDraft(JSON.stringify(selectedElement.props?.sx ?? {}, null, 2));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedElement?.id]);

  const applySxDraft = useCallback(() => {
    const parsed = safeJsonParse<Record<string, any>>(sxDraft, {});
    updateSelectedProps({ sx: parsed });
  }, [sxDraft, updateSelectedProps]);

  const showTrackHint = useMemo(() => (projectType === "dapp" ? "dApp UI Builder" : "Blockchain UI Builder"), [projectType]);

  return (
    <Box sx={{ minHeight: "calc(100vh - 64px)", px: 1 }}>
      <StepGuidance
        stepKey="step5"
        title="UI Builder"
        subtitle="STEP 6 OF 7"
        description="Design the user interface for your application. Choose from pre-built page templates and customize the layout, colors, and components to match your brand."
        steps={[
          { first: 'Choose a page template', next: 'Select from Dashboard, Marketplace, Portfolio, Governance, or a blank canvas.' },
          { first: 'Drag components onto the canvas', next: 'Add buttons, charts, tables, and cards from the left panel to build your pages.' },
          { first: 'Configure each component', next: 'Click a component to open its settings: data source, styling, actions, and visibility rules.' },
        ]}
        tip="Your entities and modules from previous steps are automatically available as data sources for tables and charts in the UI builder."
      />
      {/* Header row */}
      <Stack direction="row" alignItems="center" justifyContent="space-between" sx={{ mb: 2 }}>
        <Stack direction="row" alignItems="center" spacing={1.5}>
          <IconButton onClick={handleBack} aria-label="Back">
            <ArrowBackIcon />
          </IconButton>
          <Box>
            <Typography variant="overline" fontWeight={800} color="primary" sx={{ letterSpacing: 1 }}>STEP 6 OF 6: UI BUILDER</Typography>
            <Typography variant="h4" sx={{ fontWeight: 900, lineHeight: 1.1 }}>
              Design Your Interface
            </Typography>
            <Typography variant="body2" color="text.secondary">
              Drag components from the left panel onto the canvas. Use Templates for pre-built page layouts. Select an element to edit its properties and data bindings on the right.
            </Typography>
          </Box>
        </Stack>

        <Stack direction="row" spacing={1}>
          <Button variant="outlined" startIcon={<SaveIcon />} onClick={saveUi} sx={{ borderRadius: 999 }}>
            Save
          </Button>
          <Button variant="contained" onClick={handleNext} sx={{ borderRadius: 999 }} disabled={!goNext}>
            Next
          </Button>
        </Stack>
      </Stack>

      {/* Main builder layout (Fix #3 + #4): 3 columns, canvas biggest */}
      <DragDropContext onDragEnd={onDragEnd}>
        <Box sx={{ display: "flex", gap: 2, alignItems: "stretch" }}>
          {/* Left: Palette (Fix #5) */}
          <Paper
            sx={{
              width: 320,
              minWidth: 320,
              borderRadius: 4,
              p: 1.5,
              border: "1px solid rgba(255,255,255,0.12)",
              background: "rgba(255,255,255,0.03)",
              overflow: "hidden",
            }}
          >
            <Stack spacing={1.2}>
              <Stack direction="row" spacing={1} alignItems="center">
                <SearchIcon sx={{ opacity: 0.7 }} />
                <TextField
                  value={paletteQuery}
                  onChange={(e) => setPaletteQuery(e.target.value)}
                  placeholder="Search"
                  size="small"
                  fullWidth
                  sx={{ "& .MuiInputBase-root": { borderRadius: 3 } }}
                />
              </Stack>

              <Tabs
                value={paletteTab}
                onChange={(_, v) => setPaletteTab(v)}
                variant="fullWidth"
                sx={{
                  minHeight: 36,
                  "& .MuiTab-root": { minHeight: 36, textTransform: "none", fontSize: 13 },
                }}
              >
                <Tab value="All" label="All" />
                <Tab value="Templates" label="Templates" />
                <Tab value="Components" label="Components" />
              </Tabs>

              <Divider sx={{ opacity: 0.25 }} />

              <Typography variant="caption" sx={{ opacity: 0.7 }}>
                Drag into canvas ({filteredPalette.length})
              </Typography>

              <Droppable droppableId="palette" isDropDisabled={true}>
                {(provided) => (
                  <Box ref={provided.innerRef} {...provided.droppableProps} sx={{ maxHeight: "68vh", overflow: "auto", pr: 0.5 }}>
                    <Stack spacing={1}>
                      {filteredPalette.map((it, index) => (
                        <Draggable key={it.id} draggableId={it.id} index={index}>
                          {(p, snap) => (
                            <Paper
                              ref={p.innerRef}
                              {...p.draggableProps}
                              {...p.dragHandleProps}
                              sx={{
                                p: 1.2,
                                borderRadius: 3,
                                border: "1px solid rgba(255,255,255,0.10)",
                                background: snap.isDragging ? "rgba(255,255,255,0.07)" : "rgba(255,255,255,0.02)",
                                cursor: "grab",
                              }}
                            >
                              <Stack spacing={0.8}>
                                <Stack direction="row" alignItems="center" justifyContent="space-between">
                                  <Typography sx={{ fontWeight: 800, fontSize: 13, pl: 1.25 }}>{it.label}</Typography>
                                  <Chip size="small" label={it.category} />
                                </Stack>

                                <Box sx={{ opacity: 0.95 }}>
                                  {it.kind === "template" ? (
                                    <Typography variant="body2" sx={{ opacity: 0.8 }}>
                                      Drops a working multi-component layout
                                    </Typography>
                                  ) : (
                                    renderElement({ id: "preview", type: it.type!, props: it.preview ?? {} }, undefined)
                                  )}
                                </Box>
                              </Stack>
                            </Paper>
                          )}
                        </Draggable>
                      ))}
                      {provided.placeholder}
                    </Stack>
                  </Box>
                )}
              </Droppable>
            </Stack>
          </Paper>

          {/* Center: Canvas (big) */}
          <Paper
            sx={{
              flex: 1,
              borderRadius: 4,
              p: 1.5,
              border: "1px solid rgba(255,255,255,0.12)",
              background: "rgba(255,255,255,0.02)",
              overflow: "hidden",
              minHeight: "78vh",
            }}
          >
            <Stack direction="row" alignItems="center" justifyContent="space-between" sx={{ mb: 1 }}>
              <Stack direction="row" spacing={1} alignItems="center">
                <Typography sx={{ fontWeight: 900, pl: 1.25 }}>Canvas</Typography>
                <Chip size="small" label={pageName} />
              </Stack>

              <Stack direction="row" spacing={1} alignItems="center">
                <Button size="small" variant="outlined" startIcon={<AddIcon />} onClick={addPage} sx={{ borderRadius: 999 }}>
                  Add Page
                </Button>
                <TextField
                  size="small"
                  value={pageName}
                  onChange={(e) => renameActivePage(e.target.value)}
                  sx={{ width: 180, "& .MuiInputBase-root": { borderRadius: 3 } }}
                />
                <Tooltip title={ui.pages.length <= 1 ? "At least one page is required" : "Delete page"}>
                  <span>
                    <IconButton onClick={deleteActivePage} disabled={ui.pages.length <= 1}>
                      <DeleteIcon />
                    </IconButton>
                  </span>
                </Tooltip>
              </Stack>
            </Stack>

            <Divider sx={{ opacity: 0.18, mb: 1.5 }} />

            <Droppable droppableId="canvas">
              {(provided, snap) => (
                <Box
                  ref={provided.innerRef}
                  {...provided.droppableProps}
                  sx={{
                    height: "calc(78vh - 96px)",
                    borderRadius: 4,
                    border: snap.isDraggingOver ? "1px dashed rgba(255,255,255,0.35)" : "1px dashed rgba(255,255,255,0.14)",
                    background: snap.isDraggingOver ? "rgba(255,255,255,0.03)" : "transparent",
                    overflow: "auto",
                    p: 1.5,
                  }}
                >
                  {activeElements.length === 0 ? (
                    <Stack alignItems="center" justifyContent="center" sx={{ height: "100%", opacity: 0.75 }} spacing={1}>
                      <Typography sx={{ fontWeight: 900, pl: 1.25 }}>Drop templates/components here</Typography>
                      <Typography variant="body2" color="text.secondary">
                        Templates create a full working layout. Components add single elements.
                      </Typography>
                    </Stack>
                  ) : null}

                  <Box sx={{ width: "100%" }}>
                    <RGL
                      className="layout"
                      layout={activeLayout}
                      cols={12}
                      rowHeight={26}
                      onLayoutChange={onLayoutChange}
                      draggableHandle=".cerulea-drag-handle"
                      compactType="vertical"
                      preventCollision={false}
                    >
                      {activeElements.map((el) => {
                        const binding = ui.bindings[el.id];
                        const isSel = el.id === selectedId;
                        const sx = el.props?.sx ?? {};
                        return (
                          <Box key={el.id} sx={{ height: "100%" }}>
                            <Paper
                              onClick={() => setSelectedId(el.id)}
                              sx={{
                                height: "100%",
                                borderRadius: 4,
                                p: 1.2,
                                border: isSel ? "1px solid rgba(120,180,255,0.75)" : "1px solid rgba(255,255,255,0.12)",
                                background: isSel ? "rgba(120,180,255,0.06)" : "rgba(255,255,255,0.02)",
                                overflow: "hidden",
                                position: "relative",
                                ...sx,
                              }}
                            >
                              <Box
                                className="cerulea-drag-handle"
                                sx={{
                                  position: "absolute",
                                  left: 10,
                                  top: 8,
                                  fontSize: 11,
                                  opacity: 0.75,
                                  cursor: "move",
                                  userSelect: "none",
                                }}
                              >
                                {el.type}
                              </Box>

                              <Box sx={{ pt: 2 }}>{renderElement(el, binding)}</Box>
                            </Paper>
                          </Box>
                        );
                      })}
                    </RGL>
                  </Box>

                  {provided.placeholder}
                </Box>
              )}
            </Droppable>
          </Paper>

          {/* Right: Inspector */}
          <Paper
            sx={{
              width: 360,
              minWidth: 360,
              borderRadius: 4,
              p: 1.5,
              border: "1px solid rgba(255,255,255,0.12)",
              background: "rgba(255,255,255,0.03)",
              overflow: "hidden",
            }}
          >
            <Typography sx={{ fontWeight: 900, mb: 1, pl: 2.25 }}>Inspector</Typography>
            <Divider sx={{ opacity: 0.18, mb: 1.5 }} />

            {!selectedElement ? (
              <Typography variant="body2" color="text.secondary">
                Select a component on the canvas to edit properties, styles, and bindings.
              </Typography>
            ) : (
              <Stack spacing={1.4} sx={{ maxHeight: "72vh", overflow: "auto", pr: 0.5 }}>
                <Stack direction="row" alignItems="center" justifyContent="space-between">
                  <Typography sx={{ fontWeight: 900 }}>{selectedElement.type}</Typography>
                  <Button size="small" variant="outlined" onClick={deleteSelected} startIcon={<DeleteIcon />} sx={{ borderRadius: 999 }}>
                    Delete
                  </Button>
                </Stack>

                <Divider sx={{ opacity: 0.18 }} />

                {/* Minimal type-specific properties */}
                {(selectedElement.type === "Heading" || selectedElement.type === "Text") && (
                  <Stack spacing={1}>
                    <TextField
                      label="Text"
                      value={selectedElement.props?.text ?? ""}
                      onChange={(e) => updateSelectedProps({ text: e.target.value })}
                      size="small"
                      fullWidth
                      sx={{ "& .MuiInputBase-root": { borderRadius: 3 } }}
                    />
                    <TextField
                      label="Variant"
                      value={selectedElement.props?.variant ?? (selectedElement.type === "Heading" ? "h5" : "body2")}
                      onChange={(e) => updateSelectedProps({ variant: e.target.value })}
                      size="small"
                      fullWidth
                      sx={{ "& .MuiInputBase-root": { borderRadius: 3 } }}
                    />
                  </Stack>
                )}

                {selectedElement.type === "Button" && (
                  <Stack spacing={1}>
                    <TextField
                      label="Label"
                      value={selectedElement.props?.label ?? ""}
                      onChange={(e) => updateSelectedProps({ label: e.target.value })}
                      size="small"
                      fullWidth
                      sx={{ "& .MuiInputBase-root": { borderRadius: 3 } }}
                    />
                    <TextField
                      label="Variant"
                      value={selectedElement.props?.variant ?? "contained"}
                      onChange={(e) => updateSelectedProps({ variant: e.target.value })}
                      size="small"
                      fullWidth
                      sx={{ "& .MuiInputBase-root": { borderRadius: 3 } }}
                    />
                    <FormControlLabel
                      control={<Switch checked={!!selectedElement.props?.fullWidth} onChange={(e) => updateSelectedProps({ fullWidth: e.target.checked })} />}
                      label="Full width"
                    />
                  </Stack>
                )}

                {(selectedElement.type === "TextField" || selectedElement.type === "Textarea") && (
                  <Stack spacing={1}>
                    <TextField
                      label="Label"
                      value={selectedElement.props?.label ?? ""}
                      onChange={(e) => updateSelectedProps({ label: e.target.value })}
                      size="small"
                      fullWidth
                      sx={{ "& .MuiInputBase-root": { borderRadius: 3 } }}
                    />
                    <TextField
                      label="Placeholder"
                      value={selectedElement.props?.placeholder ?? ""}
                      onChange={(e) => updateSelectedProps({ placeholder: e.target.value })}
                      size="small"
                      fullWidth
                      sx={{ "& .MuiInputBase-root": { borderRadius: 3 } }}
                    />
                    {selectedElement.type === "Textarea" ? (
                      <TextField
                        label="Rows"
                        value={String(selectedElement.props?.rows ?? 4)}
                        onChange={(e) => updateSelectedProps({ rows: Number(e.target.value || 4) })}
                        size="small"
                        fullWidth
                        sx={{ "& .MuiInputBase-root": { borderRadius: 3 } }}
                      />
                    ) : null}
                  </Stack>
                )}

                <Divider sx={{ opacity: 0.18 }} />
                <Typography sx={{ fontWeight: 900 }}>Bindings</Typography>

                {selectedElement.type === "Button" ? (
                  <TextField
                    label="Action binding"
                    value={selectedBinding?.kind === "action" ? selectedBinding.ref : ""}
                    onChange={(e) => {
                      const v = e.target.value.trim();
                      if (!v) return setBinding(undefined);
                      setBinding({ kind: "action", ref: v });
                    }}
                    size="small"
                    fullWidth
                    placeholder="e.g., flow:createUser"
                    sx={{ "& .MuiInputBase-root": { borderRadius: 3 } }}
                  />
                ) : selectedElement.type === "Table" ? (
                  <TextField
                    select
                    label="List binding (entity[])"
                    value={selectedBinding?.kind === "list" ? selectedBinding.ref : ""}
                    onChange={(e) => {
                      const v = String(e.target.value || "");
                      if (!v) return setBinding(undefined);
                      setBinding({ kind: "list", ref: v });
                    }}
                    size="small"
                    fullWidth
                    sx={{ "& .MuiInputBase-root": { borderRadius: 3 } }}
                  >
                    {allBindingOptions
                      .filter((x) => x.value.endsWith("[]"))
                      .map((o) => (
                        <MenuItem key={o.value} value={o.value}>
                          {o.label}
                        </MenuItem>
                      ))}
                  </TextField>
                ) : selectedElement.type === "TextField" || selectedElement.type === "Textarea" ? (
                  <TextField
                    select
                    label="Value binding (entity.field)"
                    value={selectedBinding?.kind === "value" ? selectedBinding.ref : ""}
                    onChange={(e) => {
                      const v = String(e.target.value || "");
                      if (!v) return setBinding(undefined);
                      setBinding({ kind: "value", ref: v });
                    }}
                    size="small"
                    fullWidth
                    sx={{ "& .MuiInputBase-root": { borderRadius: 3 } }}
                  >
                    {allBindingOptions
                      .filter((x) => x.value && !x.value.endsWith("[]"))
                      .map((o) => (
                        <MenuItem key={o.value} value={o.value}>
                          {o.label}
                        </MenuItem>
                      ))}
                  </TextField>
                ) : selectedElement.type === "Heading" || selectedElement.type === "Text" ? (
                  <TextField
                    select
                    label="Text binding (entity.field)"
                    value={selectedBinding?.kind === "text" ? selectedBinding.ref : ""}
                    onChange={(e) => {
                      const v = String(e.target.value || "");
                      if (!v) return setBinding(undefined);
                      setBinding({ kind: "text", ref: v });
                    }}
                    size="small"
                    fullWidth
                    sx={{ "& .MuiInputBase-root": { borderRadius: 3 } }}
                  >
                    {allBindingOptions
                      .filter((x) => x.value && !x.value.endsWith("[]"))
                      .map((o) => (
                        <MenuItem key={o.value} value={o.value}>
                          {o.label}
                        </MenuItem>
                      ))}
                  </TextField>
                ) : (
                  <Typography variant="body2" color="text.secondary">
                    No bindings for this component yet.
                  </Typography>
                )}

                <Divider sx={{ opacity: 0.18 }} />
                <Typography sx={{ fontWeight: 900 }}>Styles</Typography>

                <TextField
                  label="sx (JSON)"
                  value={sxDraft}
                  onChange={(e) => setSxDraft(e.target.value)}
                  size="small"
                  fullWidth
                  multiline
                  minRows={6}
                  sx={{
                    "& .MuiInputBase-root": { borderRadius: 3 },
                    fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
                  }}
                />
                <Button variant="outlined" onClick={applySxDraft} sx={{ borderRadius: 999 }}>
                  Apply sx
                </Button>
              </Stack>
            )}
          </Paper>
        </Box>
      </DragDropContext>

      <Snackbar open={toast.open} autoHideDuration={2500} onClose={() => setToast((p) => ({ ...p, open: false }))}>
        <Alert severity={toast.kind} variant="filled" sx={{ width: "100%" }}>
          {toast.msg}
        </Alert>
      </Snackbar>
    </Box>
  );
}
