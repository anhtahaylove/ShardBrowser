import { useEffect, useMemo, useState } from "react";
import { Button, DialogModal, cn } from "@proxyshard/shardx-ui-kit";
import { Topbar } from "../../shared/ui/Topbar";
import { useStoreChanged } from "../../shared/hooks/useStoreChanged";
import { Field } from "../../shared/ui/Field";
import { CSSelect } from "../../shared/ui/CSSelect";
import { AddIcon, DeleteIcon, NavBookmarksIcon } from "../../shared/icons";
import { useBookmarks, emptyBookmark, type Bookmark } from "../../entities/bookmark";
import { useFolders, useProfile } from "../../entities/profile";

function Editor({ initial, folders, onClose }: {
  initial: Bookmark;
  folders: string[];
  onClose: () => void;
}) {
  const [b, setB] = useState(initial);
  const save = useBookmarks((s) => s.save);
  return (
    <DialogModal
      open
      onClose={onClose}
      title={initial.id ? "Edit bookmark" : "New bookmark"}
      confirmLabel="Save"
      onConfirm={() => save(b)}
      cancelLabel="Cancel"
      onCancel={onClose}
    >
      <div className="flex w-[420px] flex-col gap-3 py-4">
        <Field
          label="URL"
          value={b.url}
          onChange={(v) => setB({ ...b, url: v })}
          placeholder="facebook.com/ads/manager"
          mono
        />
        <Field
          label="Title"
          value={b.title}
          onChange={(v) => setB({ ...b, title: v })}
          placeholder="Blank uses the address itself"
        />
        <CSSelect
          title="Folder"
          value={b.folder}
          onChange={(v) => setB({ ...b, folder: v })}
          isSearchable={folders.length > 8}
          options={[
            { value: "", label: "Every profile" },
            ...folders.map((f) => ({ value: f, label: f })),
          ]}
        />
        <p className="m-0 text-paragraph-xs text-text-soft-400">
          Appears in the bookmarks bar of every profile in that folder, in a folder
          called <strong>ShardX</strong>, from their next launch. The operator's own
          bookmarks are left alone.
        </p>
      </div>
    </DialogModal>
  );
}

export function BookmarksPage() {
  const init = useBookmarks((s) => s.init);
  const items = useBookmarks((s) => s.items);
  const editing = useBookmarks((s) => s.editing);
  const setEditing = useBookmarks((s) => s.setEditing);
  const remove = useBookmarks((s) => s.remove);
  const search = useBookmarks((s) => s.search);
  const setSearch = useBookmarks((s) => s.setSearch);
  const folder = useBookmarks((s) => s.folder);
  const setFolder = useBookmarks((s) => s.setFolder);

  const initProfiles = useProfile((s) => s.init);
  const folders = useFolders();
  const profiles = useProfile((s) => s.profiles);

  const reload = useBookmarks((s) => s.reload);
  useEffect(() => { init(); initProfiles(); }, [init, initProfiles]);
  useStoreChanged(reload);

  const shown = useMemo(() => {
    const q = search.trim().toLowerCase();
    return items.filter(
      (b) =>
        (folder === "all" || b.folder === (folder === "__any__" ? "" : folder)) &&
        (!q || b.title.toLowerCase().includes(q) || b.url.toLowerCase().includes(q)),
    );
  }, [items, search, folder]);

  const countIn = (f: string) =>
    f === "" ? profiles.length : profiles.filter((p) => p.folder === f).length;

  const tabs = [
    { id: "all", label: "All" },
    { id: "__any__", label: "Every profile" },
    ...folders.map((f) => ({ id: f, label: f })),
  ];

  return (
    <section className="flex flex-col">
      <Topbar crumbs={["Library", "Bookmarks"]} search={search} onSearch={setSearch} />

      <div className="mb-3.5 flex items-end justify-between gap-4">
        <div className="flex min-w-0 flex-1 flex-col gap-3.5">
          <div>
            <h1 className="m-0 text-title-h5 text-text-strong-950">Bookmarks</h1>
            <p className="m-0 mt-1 max-w-[70ch] text-paragraph-xs text-text-soft-400">
              A site bound to a folder shows up in every profile in that folder.
            </p>
          </div>
          <div className="flex flex-wrap gap-1.5">
            {tabs.map((t) => (
              <button
                key={t.id}
                type="button"
                onClick={() => setFolder(t.id)}
                className={cn(
                  "rounded-6 px-2.5 py-1 text-label-xs ring-1 ring-inset transition-colors",
                  folder === t.id
                    ? "bg-primary-alpha-10 text-primary-base ring-primary-alpha-24"
                    : "text-text-sub-600 ring-stroke-soft-200 hover:bg-bg-weak-50",
                )}
              >
                {t.label}
              </button>
            ))}
          </div>
        </div>
        <Button
          variant="primary" mode="filled" size="small"
          leftIcon={<AddIcon className="size-4" />}
          onClick={() => setEditing(emptyBookmark(folder === "all" || folder === "__any__" ? "" : folder))}
        >
          New bookmark
        </Button>
      </div>

      <div className="overflow-hidden rounded-12 bg-bg-white-0 shadow-[var(--shadow-xs)] ring-1 ring-inset ring-stroke-soft-200">
        {shown.map((b) => (
          <div
            key={b.id}
            className="grid grid-cols-[1fr_1fr_200px_100px] items-center gap-3 border-t border-stroke-soft-200 px-4 py-2.5 first:border-t-0 transition-colors hover:bg-bg-weak-50"
          >
            <div
              className="min-w-0 cursor-pointer truncate text-label-xs text-text-strong-950 transition-colors hover:text-primary-base"
              onClick={() => setEditing(b)}
              title="Edit"
            >
              {b.title || b.url}
            </div>
            <div className="mono min-w-0 truncate text-paragraph-xs text-text-sub-600" title={b.url}>
              {b.url}
            </div>
            <div className="truncate text-paragraph-xs text-text-sub-600">
              {b.folder
                ? `${b.folder} · ${countIn(b.folder)} profile${countIn(b.folder) === 1 ? "" : "s"}`
                : `Every profile · ${profiles.length}`}
            </div>
            <div className="flex justify-end">
              <Button
                variant="error" mode="ghost" size="2xsmall"
                leftIcon={<DeleteIcon className="size-3.5" />}
                onClick={() => remove(b)}
              >
                Delete
              </Button>
            </div>
          </div>
        ))}
        {shown.length === 0 && (
          <div className="flex flex-col items-center gap-2.5 px-6 py-14 text-center">
            <div className="grid size-14 place-items-center rounded-[14px] bg-primary-alpha-10 text-primary-base ring-1 ring-inset ring-primary-alpha-24">
              <NavBookmarksIcon className="size-6" />
            </div>
            <h3 className="m-0 text-label-sm text-text-strong-950">
              {items.length === 0 ? "No bookmarks yet" : "Nothing in this folder"}
            </h3>
            <p className="m-0 max-w-[420px] text-paragraph-sm text-text-sub-600">
              Add a site and bind it to a folder; every profile in that folder picks
              it up on its next launch.
            </p>
          </div>
        )}
      </div>

      {editing && (
        <Editor initial={editing} folders={folders} onClose={() => setEditing(null)} />
      )}
    </section>
  );
}
