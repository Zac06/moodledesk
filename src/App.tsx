import { useCallback, useEffect, useMemo, useState } from "react";
import DOMPurify from "dompurify";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { open } from "@tauri-apps/plugin-dialog";
import * as api from "./api";

const P: Record<string, string> = {
  file: "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z M14 2v6h6",
  link: "M10 13a5 5 0 0 0 7.5.5l3-3a5 5 0 0 0-7-7l-1.7 1.7 M14 11a5 5 0 0 0-7.5-.5l-3 3a5 5 0 0 0 7 7l1.7-1.7",
  chat: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z",
  check: "M9 11l3 3L22 4 M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11",
  folder: "M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z",
  book: "M4 19.5A2.5 2.5 0 0 1 6.5 17H20V2H6.5A2.5 2.5 0 0 0 4 4.5z",
  down: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4 M7 10l5 5 5-5 M12 15V3",
  back: "M19 12H5 M12 19l-7-7 7-7",
  out: "M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4 M16 17l5-5-5-5 M21 12H9",
  chev: "M6 9l6 6 6-6",
  dots: "M12 12h.01 M19 12h.01 M5 12h.01",
  star: "M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01z",
  eyeoff: "M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94 M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19 M1 1l22 22",
  eye: "M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z",
  ext: "M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6 M15 3h6v6 M10 14L21 3",
};
const Icon = ({ n, s = 18 }: { n: string; s?: number }) => (
  <svg width={s} height={s} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d={P[n]} /></svg>
);
const modIcon = (m: string) => ({ resource: "file", url: "link", forum: "chat", folder: "folder", assign: "check", quiz: "check", choice: "check", feedback: "check" } as any)[m] ?? "book";
const hue = (id: number) => (id * 47) % 360;
const size = (b: number) => (b > 1e6 ? (b / 1e6).toFixed(1) + " MB" : Math.max(1, Math.round(b / 1e3)) + " KB");
const initials = (n: string) =>
  n.split(/[\s\-–]+/).filter((w) => /^\p{L}{3,}/u.test(w)).slice(0, 2).map((w) => w[0]).join("").toUpperCase() || n[0]?.toUpperCase() || "?";
const grad = (id: number) => `linear-gradient(135deg,hsl(${hue(id)} 70% 55%),hsl(${hue(id) + 40} 70% 42%))`;
// Moodle's auto-generated covers are data: URIs -> keep our gradient; real images need the token
const realImage = (u: string | undefined, token: string) => {
  if (!u || u.startsWith("data:")) return null;
  const w = u.includes("/webservice/pluginfile.php") ? u : u.replace("/pluginfile.php", "/webservice/pluginfile.php");
  return w + (w.includes("?") ? "&" : "?") + "token=" + token;
};
const FILTERS: [api.Filter, string][] = [["all", "All"], ["inprogress", "In progress"], ["future", "Future"], ["past", "Ended"], ["favourites", "Favourites"], ["hidden", "Hidden"]];

function Login({ onDone }: { onDone: () => void }) {
  const [url, setUrl] = useState(""); const [site, setSite] = useState<{ site: string; config: any } | null>(null);
  const [user, setUser] = useState(""); const [pass, setPass] = useState(""); const [msg, setMsg] = useState(""); const [busy, setBusy] = useState(false);
  useEffect(() => {
    const a = listen("sso-complete", onDone); const b = listen<string>("sso-error", (e) => { setMsg(e.payload); setBusy(false); });
    return () => { a.then((f) => f()); b.then((f) => f()); };
  }, []);
  const sso = site && (site.config.typeoflogin ?? 1) !== 1;
  const connect = async () => { setMsg(""); setBusy(true); try { setSite(await api.resolveSite(url)); } catch (e) { setMsg(String(e)); } setBusy(false); };
  return (
    <div className="login">
      <div className="hero"><div className="logo">M</div><h1>MoodleDesk</h1><p>All your courses and materials, natively on your desktop.</p></div>
      <div className="panel">
        <h2>{site ? site.config.sitename : "Connect to your university"}</h2>
        {!site ? (<>
          <label>Moodle address</label>
          <input autoFocus placeholder="moodle.university.edu" value={url} onChange={(e) => setUrl(e.target.value)} onKeyDown={(e) => e.key === "Enter" && connect()} />
          <button className="primary" disabled={busy || !url} onClick={connect}>{busy ? "Connecting…" : "Continue"}</button>
        </>) : sso ? (<>
          <p className="muted">You'll sign in through your institution in your browser, then return here.</p>
          <button className="primary" onClick={() => { api.startSso(site.site); setMsg("Waiting for the browser…"); }}>Sign in with your institution</button>
        </>) : (<>
          <label>Username</label><input value={user} onChange={(e) => setUser(e.target.value)} />
          <label>Password</label><input type="password" value={pass} onChange={(e) => setPass(e.target.value)} />
          <button className="primary" onClick={() => api.loginPassword(site.site, user, pass).then(onDone).catch((e) => setMsg(String(e)))}>Sign in</button>
        </>)}
        {site && <button className="ghost" onClick={() => { setSite(null); setMsg(""); }}>Use a different site</button>}
        {msg && <p className="msg">{msg}</p>}
      </div>
    </div>
  );
}

const hasContent = (h?: string) => !!h && (/<(img|a)\b/i.test(h) || h.replace(/<[^>]*>|&nbsp;|\s/g, "").length > 0);

// Moodle HTML (section summaries, labels): sanitised, files get the token, link clicks are routed through onLink
function Html({ html, token, onLink }: { html: string; token: string; onLink: (href: string) => void }) {
  const clean = useMemo(() => DOMPurify.sanitize(html, { FORBID_ATTR: ["style", "class", "id"], FORBID_TAGS: ["style", "form", "input", "button"] })
    .replace(/src="([^"]*\/webservice\/pluginfile\.php[^"]*)"/g, (_, u) => `src="${u}${u.includes("?") ? "&amp;" : "?"}token=${token}"`), [html, token]);
  return <div className="html" dangerouslySetInnerHTML={{ __html: clean }}
    onClick={(e) => { const a = (e.target as HTMLElement).closest("a"); if (!a) return; e.preventDefault(); const h = a.getAttribute("href"); if (h) onLink(h); }} />;
}

function CourseView({ c, site, token, ensureDir, notify, onLink }: { c: api.Course; site: string; token: string; ensureDir: () => Promise<string | null>; notify: (m: string) => void; onLink: (href: string) => void }) {
  const [secs, setSecs] = useState<api.Section[] | null>(null); const [err, setErr] = useState(""); const [closed, setClosed] = useState<Record<number, boolean>>({});
  useEffect(() => { setSecs(null); setErr(""); api.courseContents(c.id).then(setSecs).catch((e) => setErr(String(e))); }, [c.id]);
  const getFile = async (f: api.FileItem) => {
    const dir = await ensureDir(); if (!dir) return;
    notify("Downloading " + f.filename + "…");
    try { await api.download(f.fileurl, dir, c.shortname || c.fullname, f.filename); notify("Opened " + f.filename); } catch (e) { notify("Download failed: " + e); }
  };
  if (err) return (
    <div className="sec pad"><p>This course can't be shown in the app (you may not be enrolled).</p><small>{err}</small><br />
      <button className="primary" onClick={() => openUrl(`${site}/course/view.php?id=${c.id}`)}>Open in browser</button></div>);
  if (!secs) return <div className="muted pad">Loading course…</div>;
  return (<div className="stack">{secs.filter((s) => s.modules.length || hasContent(s.summary)).map((s) => (
    <section className="sec" key={s.id}>
      <button className="sechead" onClick={() => setClosed({ ...closed, [s.id]: !closed[s.id] })}>
        <span>{s.name || "General"}</span><span className={closed[s.id] ? "chev shut" : "chev"}><Icon n="chev" /></span>
      </button>
      {!closed[s.id] && hasContent(s.summary) && <div className="summary"><Html html={s.summary!} token={token} onLink={onLink} /></div>}
      {!closed[s.id] && s.modules.map((m) => {
        if (m.modname === "label") return hasContent(m.description) ? <div className="summary" key={m.id}><Html html={m.description!} token={token} onLink={onLink} /></div> : null;
        const files = (m.contents ?? []).filter((f) => f.type === "file");
        if (files.length) return files.map((f) => (
          <button className="item" key={m.id + f.filename} onClick={() => getFile(f)}>
            <span className="ico"><Icon n={modIcon(m.modname)} /></span><span className="grow">{m.modname === "folder" ? `${m.name} / ${f.filename}` : m.name}<small>{f.filename} · {size(f.filesize)}</small></span><Icon n="down" />
          </button>));
        const isUrl = m.modname === "url"; const target = isUrl ? m.contents?.[0]?.fileurl ?? m.url : m.url;
        return (
          <button className="item" key={m.id} onClick={() => target && onLink(target)}>
            <span className="ico"><Icon n={modIcon(m.modname)} /></span><span className="grow">{m.name}<small>{isUrl ? "link" : m.modname + " · opens in browser"}</small></span><Icon n="link" s={16} />
          </button>);
      })}
    </section>))}</div>);
}

export default function App() {
  const [session, setSession] = useState<api.Session | null | undefined>(undefined);
  const [info, setInfo] = useState<api.Info | null>(null); const [courses, setCourses] = useState<api.Course[]>([]);
  const [stack, setStack] = useState<api.Course[]>([]); const [q, setQ] = useState("");
  const open_ = stack[stack.length - 1] ?? null;
  const setOpen = (c: api.Course | null) => setStack(c ? [c] : []);
  const [dir, setDir] = useState(localStorage.getItem("dlDir")); const [toast, setToast] = useState("");
  const notify = useCallback((m: string) => { setToast(m); setTimeout(() => setToast(""), 3500); }, []);
  const refresh = () => api.getSession().then(setSession);
  useEffect(() => { refresh(); }, []);
  const [filter, setFilter] = useState<api.Filter>("all"); const [menu, setMenu] = useState<number | null>(null); const [loading, setLoading] = useState(false);
  const load = useCallback((f: api.Filter) => {
    setLoading(true);
    return api.listCourses(f).then(setCourses).catch((e) => notify(String(e))).finally(() => setLoading(false));
  }, [notify]);
  useEffect(() => { if (session) load(filter); }, [session, filter]);
  useEffect(() => { if (session) api.siteInfo().then(setInfo).catch(() => { api.logout(); setSession(null); }); }, [session]);
  const act = async (c: api.Course, what: "fav" | "hide") => {
    setMenu(null);
    try {
      if (what === "fav") await api.setFavourite(c.id, !c.isfavourite);
      else await api.setHidden(info!.userid, c.id, filter !== "hidden" && !c.hidden);
      await load(filter);
    } catch (e) { notify("Couldn't update course: " + e); }
  };
  const openCourseById = async (id: number) => {
    if (open_?.id === id) return;
    try {
      const c = courses.find((x) => x.id === id) ?? (await api.courseById(id));
      if (!c) throw new Error("not found");
      setStack((st) => [...st, c]);
    } catch { openUrl(`${session!.site}/course/view.php?id=${id}`); }
  };
  // same-site course links open in the app; everything else in the system browser
  const handleLink = (href: string) => {
    if (!session) return;
    let u: URL; try { u = new URL(href, session.site + "/"); } catch { return; }
    if (!/^https?:$/.test(u.protocol)) return;
    const id = u.origin === new URL(session.site).origin && u.pathname.endsWith("/course/view.php") ? Number(u.searchParams.get("id")) : 0;
    if (id) openCourseById(id); else openUrl(u.toString());
  };
  const pick = async () => { const p = await open({ directory: true, title: "Choose download folder" }); if (typeof p === "string") { localStorage.setItem("dlDir", p); setDir(p); return p; } return null; };
  const ensureDir = async () => dir ?? pick();

  if (session === undefined) return null;
  if (!session) return <Login onDone={refresh} />;
  const shown = courses.filter((c) => c.fullname.toLowerCase().includes(q.toLowerCase()));
  return (
    <div className="app">
      <aside>
        <div className="brand"><div className="logo sm">M</div>MoodleDesk</div>
        <nav><button className={!open_ ? "on" : ""} onClick={() => setOpen(null)}><Icon n="book" />My courses</button></nav>
        <div className="grow" />
        <button className="folder" onClick={pick}><Icon n="folder" /><span><small>Download folder</small>{dir ? dir.split(/[\\/]/).pop() : "Not set — click to choose"}</span></button>
        <div className="user"><div className="avatar">{initials(info?.fullname ?? "?")}</div><span className="grow">{info?.fullname}</span>
          <button className="icon" title="Sign out" onClick={() => api.logout().then(() => { setOpen(null); refresh(); })}><Icon n="out" /></button></div>
      </aside>
      <main>
        {open_ ? (<>
          <div className="bar"><button className="icon" onClick={() => setStack(stack.slice(0, -1))}><Icon n="back" /></button><h1>{open_.fullname}</h1></div>
          <div className="banner" style={{ background: `linear-gradient(135deg,hsl(${hue(open_.id)} 70% 55%),hsl(${hue(open_.id) + 40} 70% 42%))` }}>{open_.shortname}</div>
          <CourseView key={open_.id} c={open_} site={session.site} token={session.token} ensureDir={ensureDir} notify={notify} onLink={handleLink} />
        </>) : (<>
          <div className="bar"><h1>My courses</h1><input className="search" placeholder="Search courses…" value={q} onChange={(e) => setQ(e.target.value)} /></div>
          <div className="tabs">{FILTERS.map(([k, l]) => <button key={k} className={filter === k ? "tab on" : "tab"} onClick={() => setFilter(k)}>{l}</button>)}</div>
          {loading ? <p className="muted pad">Loading…</p> : (
            <div className="grid" onClick={() => setMenu(null)}>{shown.map((c) => {
              const img = realImage(c.courseimage, session.token); const hid = filter === "hidden" || c.hidden;
              return (
                <div className={menu === c.id ? "course open" : "course"} role="button" key={c.id} onClick={() => setOpen(c)}>
                  <div className="cover" style={{ background: grad(c.id) }}>
                    <span>{initials(c.fullname)}</span>
                    {img && <img src={img} onError={(e) => (e.currentTarget.style.display = "none")} />}
                  </div>
                  {c.isfavourite && <span className="fav"><Icon n="star" s={15} /></span>}
                  <button className={menu === c.id ? "dots on" : "dots"} title="Options" onClick={(e) => { e.stopPropagation(); setMenu(menu === c.id ? null : c.id); }}><Icon n="dots" /></button>
                  {menu === c.id && (
                    <div className="menu" onClick={(e) => e.stopPropagation()}>
                      <button onClick={() => act(c, "fav")}><Icon n="star" s={16} />{c.isfavourite ? "Remove from favourites" : "Add to favourites"}</button>
                      <button onClick={() => act(c, "hide")}><Icon n={hid ? "eye" : "eyeoff"} s={16} />{hid ? "Show in course list" : "Hide from view"}</button>
                      <button onClick={() => { setMenu(null); openUrl(`${session.site}/course/view.php?id=${c.id}`); }}><Icon n="ext" s={16} />Open in browser</button>
                    </div>)}
                  <div className="body"><b>{c.fullname}</b>
                    {c.progress != null && <div className="prog"><i style={{ width: `${Math.round(c.progress)}%` }} /></div>}
                    {c.progress != null && <small>{Math.round(c.progress)}% complete</small>}</div>
                </div>);
            })}</div>)}
          {!shown.length && !loading && <p className="muted pad">No courses in this view.</p>}
        </>)}
      </main>
      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}
