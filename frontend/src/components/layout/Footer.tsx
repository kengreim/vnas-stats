export function Footer() {
  const year = new Date().getUTCFullYear();

  return (
    <footer class="mt-8 border-t border-border bg-secondary/30 px-6 py-4 text-sm text-muted-foreground">
      <div class="mx-auto flex max-w-6xl flex-col gap-2 md:flex-row md:items-center md:justify-between">
        <span>VNAS Stats © {year}</span>
        <span class="text-xs uppercase tracking-[0.2em]">Powered by VATSIM data</span>
      </div>
    </footer>
  );
}
