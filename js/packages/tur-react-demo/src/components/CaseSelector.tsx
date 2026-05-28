import { caseNames, getCaseFiles } from "../lib/cases";

interface CaseSelectorProps {
    selectedCase: string | null;
    selectedFile: string;
    onSelectCase: (name: string) => void;
    onSelectFile: (fileName: string) => void;
}

export function CaseSelector({
    selectedCase,
    selectedFile,
    onSelectCase,
    onSelectFile,
}: CaseSelectorProps) {
    return (
        <div className="case-selector">
            <div className="case-selector-header">
                <h2>tur playground</h2>
            </div>
            <div className="case-selector-list">
                {caseNames.map((name) => {
                    const isActive = selectedCase === name;
                    const files = isActive ? getCaseFiles(name) : [];
                    return (
                        <div key={name} className="case-group">
                            <button
                                type="button"
                                className={`case-item ${isActive ? "active" : ""}`}
                                onClick={() => onSelectCase(name)}
                            >
                                {name}
                            </button>
                            {isActive && files.length > 1 && (
                                <div className="file-tree">
                                    {files.map((file) => (
                                        <button
                                            key={file}
                                            type="button"
                                            className={`file-item ${selectedFile === file ? "active" : ""}`}
                                            onClick={() => onSelectFile(file)}
                                        >
                                            {file}
                                        </button>
                                    ))}
                                </div>
                            )}
                        </div>
                    );
                })}
            </div>
        </div>
    );
}
