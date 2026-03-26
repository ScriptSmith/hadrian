import type { Meta, StoryObj } from "@storybook/react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";

import { SovereigntyFormFields, sovereigntySchema, sovereigntyDefaults } from "./sovereigntyFields";

const schema = z.object(sovereigntySchema);
type FormValues = z.infer<typeof schema>;

function SovereigntyFieldsDemo({ idPrefix = "apikey" }: { idPrefix?: string }) {
  const { register } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: sovereigntyDefaults,
  });

  return (
    <form className="max-w-2xl">
      <SovereigntyFormFields register={register} idPrefix={idPrefix} />
    </form>
  );
}

const meta: Meta<typeof SovereigntyFormFields> = {
  title: "Admin/SovereigntyFormFields",
  component: SovereigntyFormFields,
  parameters: {
    layout: "padded",
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => <SovereigntyFieldsDemo />,
};

export const CustomIdPrefix: Story = {
  render: () => <SovereigntyFieldsDemo idPrefix="self-apikey" />,
};
