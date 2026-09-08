import { Button } from "@proxyshard/shardx-ui-kit";
import { useProfile } from "../../../entities/profile";

export function FromTemplateButton() {
  const setTemplatePickerOpen = useProfile((s) => s.setTemplatePickerOpen);
  return (
    <Button variant="neutral" mode="stroke" size="small" onClick={() => setTemplatePickerOpen(true)}>
      From template
    </Button>
  );
}
