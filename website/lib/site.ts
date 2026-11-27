export const REPO = "https://github.com/ultron-hq/clickhand";

/** The real star count from GitHub, refreshed hourly; null if GitHub doesn't answer. */
export async function getStars(): Promise<number | null> {
  try {
    const res = await fetch("https://api.github.com/repos/ultron-hq/clickhand", {
      headers: { accept: "application/vnd.github+json" },
      next: { revalidate: 3600 },
    });
    if (!res.ok) return null;
    const j = (await res.json()) as { stargazers_count?: number };
    return typeof j.stargazers_count === "number" ? j.stargazers_count : null;
  } catch {
    return null;
  }
}
