"use client";

import { useEffect, useRef, useState } from "react";
import { DiscordIcon } from "@/components/DiscordIcon";
import { DISCORD } from "@/lib/site";
import { usePathname } from "next/navigation";
import { Menu, X, Star } from "lucide-react";

const LINKS = [
  { id: "demo", label: "Demo" },
  { id: "skills", label: "Skills" },
  { id: "hands", label: "Hands" },
  { id: "app", label: "The app" },
  { id: "safety", label: "Safety" },
];

export function Header({ repo, stars }: { repo: string; stars: number | null }) {
  const [active, setActive] = useState("");
  const [open, setOpen] = useState(false);
  const [scrolled, setScrolled] = useState(false);
  const bar = useRef<HTMLSpanElement>(null);
  const path = usePathname();

  useEffect(() => {
    // which section is on screen
    const seen = new Map<string, number>();
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) seen.set(e.target.id, e.isIntersecting ? e.intersectionRatio : 0);
        let best = "", ratio = 0;
        for (const [id, r] of seen) if (r > ratio) { best = id; ratio = r; }
        setActive(best);
      },
      { threshold: [0, 0.2, 0.4, 0.6], rootMargin: "-64px 0px -30% 0px" },
    );
    LINKS.forEach(({ id }) => { const el = document.getElementById(id); if (el) io.observe(el); });

    // reading progress, as a hairline of the brand light
    const onScroll = () => {
      const max = document.documentElement.scrollHeight - innerHeight;
      if (bar.current) bar.current.style.transform = `scaleX(${max > 0 ? scrollY / max : 0})`;
      setScrolled(scrollY > 8);
    };
    onScroll();
    addEventListener("scroll", onScroll, { passive: true });
    return () => { io.disconnect(); removeEventListener("scroll", onScroll); };
  }, []);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    addEventListener("keydown", onKey);
    return () => removeEventListener("keydown", onKey);
  }, [open]);

  return (
    <header className={`nav${scrolled ? " nav-scrolled" : ""}${open ? " nav-open" : ""}`}>
      <div className="nav-inner">
        <a className="brand" href="/" aria-label="OpenHUD, home">
          <img src="/logo.svg" alt="" width={26} height={26} />
          <span className="brand-word">OpenHUD</span>
        </a>
        <nav className="nav-links" aria-label="Main">
          {LINKS.map(({ id, label }) => (
            <a key={id} href={path === "/" ? `#${id}` : `/#${id}`} aria-current={active === id ? "true" : undefined} onClick={() => setOpen(false)}>
              {label}
            </a>
          ))}
          <a href="/docs" aria-current={path.startsWith("/docs") ? "page" : undefined} onClick={() => setOpen(false)}>Docs</a>
        </nav>
        <div className="nav-end">
          <a className="btn btn-quiet btn-sm nav-discord" href={DISCORD} aria-label="Join the OpenHUD Discord">
            <DiscordIcon size={16} />
            <span className="nav-discord-label">Discord</span>
          </a>
          <a className="btn btn-quiet btn-sm" href={repo}>
            GitHub
            {stars !== null && stars > 0 && (
              <span className="stars"><Star size={14} strokeWidth={1.5} aria-hidden="true" />{stars.toLocaleString("en-US")}</span>
            )}
          </a>
          <button
            type="button"
            className="nav-toggle"
            aria-expanded={open}
            aria-label={open ? "Close menu" : "Open menu"}
            onClick={() => setOpen((o) => !o)}
          >
            {open ? <X size={18} strokeWidth={1.5} /> : <Menu size={18} strokeWidth={1.5} />}
          </button>
        </div>
      </div>
      <span className="nav-progress" ref={bar} aria-hidden="true" />
    </header>
  );
}
