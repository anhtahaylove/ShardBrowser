import {
  BulkActionsBar,
  ImportProfilesButton,
  FromTemplateButton,
  NewProfileButton,
} from "../../features/manage-profiles";

export function ProfileToolbar() {
  return (
    <div className="flex items-center flex-none gap-2">
      <BulkActionsBar />
      <ImportProfilesButton />
      <FromTemplateButton />
      <NewProfileButton />
    </div>
  );
}
