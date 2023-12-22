import { spawn } from 'child_process';

const targets = [
    {
        command: 'cargo',
        args: ['build', '--target', 'aarch64-apple-darwin', '--release'],
    },
    {
        command: 'cargo',
        args: ['build', '--target', 'x86_64-unknown-linux-musl', '--release'],
    },
    {
        command: 'cargo',
        args: ['xwin', 'build', '--target', 'x86_64-pc-windows-msvc', '--release'],
    }
]

async function run(command, args, options) {
    return new Promise((resolve, reject) => {
        const child = spawn(command, args, options);
        child.on('exit', (code, signal) => {
            if (code === 0) {
                resolve();
            } else {
                reject(new Error(`Process exited with non-zero code: ${code} signal: ${signal}`));
            }
        });
    });
}

async function main() {
    await Promise.all(targets.map(({ command, args }) => run(command, args, {
            cwd: 'cli',
            stdio: 'inherit',
        })
    ));
}

main().catch((err) => {
    console.error(err);
    process.exit(1);
});
