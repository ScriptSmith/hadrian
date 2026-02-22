import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";
import {
  Modal,
  ModalHeader,
  ModalTitle,
  ModalDescription,
  ModalContent,
  ModalFooter,
  ModalClose,
} from "./Modal";
import { Button } from "../Button/Button";

const meta: Meta<typeof Modal> = {
  title: "UI/Modal",
  component: Modal,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

function DefaultStory() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button onClick={() => setOpen(true)}>Open Modal</Button>
      <Modal open={open} onClose={() => setOpen(false)}>
        <ModalClose onClose={() => setOpen(false)} />
        <ModalHeader>Modal Title</ModalHeader>
        <ModalContent>
          <p>This is the modal content. You can put any content here.</p>
        </ModalContent>
        <ModalFooter>
          <Button variant="secondary" onClick={() => setOpen(false)}>
            Cancel
          </Button>
          <Button onClick={() => setOpen(false)}>Confirm</Button>
        </ModalFooter>
      </Modal>
    </>
  );
}

export const Default: Story = {
  render: () => <DefaultStory />,
};

function WithDescriptionStory() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button onClick={() => setOpen(true)}>Open Modal</Button>
      <Modal open={open} onClose={() => setOpen(false)}>
        <ModalClose onClose={() => setOpen(false)} />
        <ModalHeader>
          <div>
            <ModalTitle>Delete Item</ModalTitle>
            <ModalDescription>
              This action cannot be undone. Are you sure you want to proceed?
            </ModalDescription>
          </div>
        </ModalHeader>
        <ModalFooter>
          <Button variant="secondary" onClick={() => setOpen(false)}>
            Cancel
          </Button>
          <Button variant="danger" onClick={() => setOpen(false)}>
            Delete
          </Button>
        </ModalFooter>
      </Modal>
    </>
  );
}

export const WithDescription: Story = {
  render: () => <WithDescriptionStory />,
};

function LargeContentStory() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button onClick={() => setOpen(true)}>Open Modal</Button>
      <Modal open={open} onClose={() => setOpen(false)}>
        <ModalClose onClose={() => setOpen(false)} />
        <ModalHeader>Terms and Conditions</ModalHeader>
        <ModalContent className="max-h-96 overflow-y-auto">
          {Array.from({ length: 10 }).map((_, i) => (
            <p key={i} className="mb-4">
              Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor
              incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud
              exercitation ullamco laboris.
            </p>
          ))}
        </ModalContent>
        <ModalFooter>
          <Button variant="secondary" onClick={() => setOpen(false)}>
            Decline
          </Button>
          <Button onClick={() => setOpen(false)}>Accept</Button>
        </ModalFooter>
      </Modal>
    </>
  );
}

export const LargeContent: Story = {
  render: () => <LargeContentStory />,
};
