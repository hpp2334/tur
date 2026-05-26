import { caseNames } from "../lib/cases";

interface CaseSelectorProps {
    selectedCase: string | null;
    onSelect: (name: string) => void;
}

export function CaseSelector({ selectedCase, onSelect }: CaseSelectorProps) {
    return (
        <div className="case-selector">
            <div className="case-selector-header">
                <h2>tur playground</h2>
            </div>
            <div className="case-selector-list">
                {caseNames.map((name) => (
                    <button
                        key={name}
                        type="button"
                        className={`case-item ${selectedCase === name ? "active" : ""}`}
                        onClick={() => onSelect(name)}
                    >
                        {name}
                    </button>
                ))}
            </div>
        </div>
    );
}
