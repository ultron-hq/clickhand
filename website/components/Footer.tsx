import { REPO } from "@/lib/site";

export function Footer() {
  return (
    <footer className="foot">
      <div className="foot-row">
        <span className="brand brand-sm"><img src="/logo.svg" alt="" width={18} height={18} />Ultron, open source</span>
        <nav aria-label="Footer">
          <a href="/docs">Docs</a>
          <a href={REPO}>GitHub</a>
          <a href={`${REPO}/releases`}>Releases</a>
          <a href={`${REPO}/discussions`}>Discussions</a>
          <a href={`${REPO}/blob/main/SECURITY.md`}>Security</a>
        </nav>
      </div>
      <p className="wordmark" aria-hidden="true">Ultron</p>
    </footer>
  );
}
