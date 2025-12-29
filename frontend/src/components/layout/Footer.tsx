import { GithubIcon } from "~/components/icons/GithubIcon.tsx";
import { DiscordIcon } from "~/components/icons/DiscordIcon.tsx";
import { Link } from "@tanstack/solid-router";

export const Footer = () => {
  const year = new Date().getUTCFullYear();

  return (
    <footer class="mt-8 border-t border-border bg-secondary px-6 py-4 text-sm text-muted-foreground">
      <div class="mx-auto flex max-w-6xl flex-col gap-3 md:flex-row md:items-center md:justify-between">
        <div>
          <span>vNAS Stats © {year}</span>
          <span class="no- mx-2 font-extrabold select-none">&middot;</span>
          <Link to="/privacy" class="transition-colors hover:text-primary">
            Privacy Policy
          </Link>
        </div>
        <div class="flex items-center gap-3">
          <a
            href="https://discord.gg/NaFQePmdXg"
            target="_blank"
            rel="noreferrer"
            aria-label="Join the community on Discord"
            class="text-foreground transition hover:text-primary"
          >
            <DiscordIcon class="fill-primary/50 transition-colors hover:fill-primary" />
          </a>
          <a
            href="https://github.com/kengreim/vnas-stats"
            target="_blank"
            rel="noreferrer"
            aria-label="View the project on GitHub"
            class="text-foreground transition hover:text-primary"
          >
            <GithubIcon class="fill-primary/50 transition-colors hover:fill-primary" />
          </a>
          <a href="https://vnas.vatsim.net">
            <img
              src="/images/vnas_logo_512x512.png"
              class="w-5 opacity-50 transition-opacity hover:opacity-100"
              alt="vNAS logo"
            />
          </a>
        </div>
      </div>
    </footer>
  );
};
