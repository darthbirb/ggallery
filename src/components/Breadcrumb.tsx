/** A chain of folder titles, joined by a slash. No domain knowledge — the
 *  caller resolves whatever ancestry it means to show (see `lib/folders.ts`
 *  for the item and folder cases) and hands over the titles alone.
 *
 *  Small mono segments, deliberately neither `Chip`'s pill nor a two-tone
 *  field: a folder reads as neither a tag nor a field, and looking like
 *  either would be its own confusion. */

export function Breadcrumb({ titles }: { titles: string[] }) {
  return (
    <div className="flex flex-wrap items-center gap-1 font-mono text-[12px] text-fg-dim">
      {titles.map((title, index) => (
        <span key={index} className="flex items-center gap-1">
          {index > 0 && <span aria-hidden>/</span>}
          <span className="truncate rounded-[3px] bg-raised px-1.5 py-0.5">{title}</span>
        </span>
      ))}
    </div>
  );
}
