import React from 'react';
import { FileManager } from '../components/files/FileManager';

export interface FileManagerPageProps {
  initialPath?: string;
}

export const FileManagerPage: React.FC<FileManagerPageProps> = ({ initialPath = '' }) => {
  return (
    <div className="p-4 sm:p-6 space-y-4 max-w-[1400px]">
      <div>
        <h1 className="text-xl font-bold tracking-tight text-[var(--foreground)]">
          Pengelola Berkas
        </h1>
        <p className="text-xs text-[var(--muted-foreground)]">
          Jelajahi, sunting, unggah, dan kelola seluruh berkas hosting dalam ruang terisolasi tenant
        </p>
      </div>

      <FileManager initialPath={initialPath} />
    </div>
  );
};

export default FileManagerPage;
