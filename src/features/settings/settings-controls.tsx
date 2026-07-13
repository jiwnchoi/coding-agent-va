export function SettingRow({
  control,
  description,
  title,
}: {
  control: React.ReactNode;
  description: string;
  title: string;
}) {
  return (
    <div className="border-border flex min-h-20 items-center justify-between gap-8 border-b py-4 last:border-b-0">
      <div className="min-w-0">
        <h2 className="text-sm font-medium">{title}</h2>
        <p className="text-muted-foreground mt-1 text-sm leading-5">{description}</p>
      </div>
      <div className="w-56 shrink-0">{control}</div>
    </div>
  );
}

export function SelectControl<T extends string | number>({
  value,
  choices,
  onChange,
}: {
  value: T;
  choices: { label: string; value: T }[];
  onChange: (value: T) => void;
}) {
  return (
    <select
      value={value}
      onChange={(event) => {
        const choice = choices.find(
          ({ value: choiceValue }) => String(choiceValue) === event.target.value
        );
        if (choice) onChange(choice.value);
      }}
      className="border-input bg-background focus:ring-ring/50 h-9 w-full rounded-md border px-3 text-sm outline-none focus:ring-2">
      {choices.map((choice) => (
        <option key={choice.value} value={choice.value}>
          {choice.label}
        </option>
      ))}
    </select>
  );
}
