# Beginner path

The beginner path gets a new writer from an empty project to a playable choice without opening code. Projects stay ordinary Kairos files, and the script remains the source of truth. Code appears when the writer asks for it.

## Starter templates

The New project dialog offers three starters:

- **Blank story** creates an empty visual story and is the default. It opens the graph with the first-run checklist.
- **Kinetic novel** creates a linear scene with a narrator line and a simple ending.
- **Branching romance** creates two choices, an affection variable and a conditional ending.

The templates live under `examples/beginner-templates/` and use the same editable script files as every other project. The template picker collects a project name and parent folder; Kairos creates the project folder there.

## First project

The editor shows a dismissible checklist until the writer closes it:

1. Add a character.
2. Write a line.
3. Add a choice.
4. Play the story.

The editor derives progress from the open project's scripts and play state. Dismissing the checklist affects that project only; it does not alter story files.

## Code visibility

Projects created from these templates start with **Show code** off. The editor hides `.kai` entries in Files, script tabs and Monaco until the writer enables **Show code** in Settings under Workspace. Other projects keep code visible by default. The setting is stored per project in editor settings, not in the game script.

Showing code does not create a second source of truth. The visual editor and script editor continue to read and write the same `.kai` files.
