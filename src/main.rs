// Created: 2025/1/5
//  Edited: 2025/1/6
//  Author: Alexander Bass
//
// The program which follows acts as a simple JIT compiler.
// A sequence of arithmetic operations is fed to a jit function which converts
// them into executable x86_64 machine code. Then, the machine code is sent
// to a run function which executes the machine code and receives a return
// value from it. The operations are very simple and all act upon one variable.
//
// "+": Increment variable
// "-": Decrement variable
// "*": Double variable
// "/": Halve variable
//
// Some example sequences and their outputs include:
// "+":  1
// "-": -1
// "++*": 4
// "++*-/": 1
//
// This has been tested and works on x86_64 Linux. It should work on Windows
// and other OSes, but will certainly not work on other CPU architectures.
//
// The region library is used as a cross-platform way to allocate executable memory.
//
// This was inspired by a the blog post <https://ochagavia.nl/blog/the-jit-calculator-challenge/>
//
//
use region::Protection;

fn main() {
    let p = jit("+ + * - /");
    let r = run(&p);
    println!("{r}");
}

/// Compile the sequence of instructions into working x86_64 machine code
/// following the C calling convention. The type of the function
/// produced (in C notation) is: `int64_t f()`
fn jit(program: &str) -> Vec<u8> {
    // Step 1, tokenize the string into operations
    enum Op {
        Plus,
        Minus,
        Star,
        Slash,
    }

    let mut tokens: Vec<Op> = Vec::new();

    for c in program.chars() {
        let t = match c {
            '+' => Op::Plus,
            '-' => Op::Minus,
            '*' => Op::Star,
            '/' => Op::Slash,
            ' ' | '\n' => continue,
            e => panic!("Unknown character in program string: {e}"),
        };
        tokens.push(t);
    }
    assert!(!tokens.is_empty());

    // Step 2: Compile
    // The tokens are compiled to a sequence of instructions.

    let mut machine_code: Vec<u8> = Vec::new();
    // Set working 64-bit register (rcx) to zero by xoring it with itself
    // `xor %rcx, %rcx`
    machine_code.extend_from_slice(&[0x48, 0x31, 0xc9]);

    for token in tokens {
        let m: &[u8] = match token {
            // Increment the working register by 1
            // `inc %rcx`
            Op::Plus => &[0x48, 0xff, 0xc1],
            // Decrement the working register by 1
            // `dec %rcx`
            Op::Minus => &[0x48, 0xff, 0xc9],
            // Multiply the working register by 2
            // `imul $0x02, %rcx`
            Op::Star => &[0x48, 0x6b, 0xc9, 0x02],
            // Copy the value in the working register (rcx) to rax
            // `mov  %rcx, %rax`
            // Copy the divisor (2) into register r8
            // `mov $0x02, %r8`
            // Just google this one
            // `cqto`
            // Divide the value in rax by the value in r8, store result to rax.
            // `idivq %r8`
            // Move result (currently in rax) back into working register (rcx)
            // `mov %rax, %rcx`
            Op::Slash => &[
                0x48, 0x89, 0xC8, 0x49, 0xc7, 0xc0, 0x02, 0x00, 0x00, 0x00, 0x48, 0x99, 0x49, 0xF7,
                0xF8, 0x48, 0x89, 0xC1,
            ],
        };
        machine_code.extend_from_slice(m);
    }
    // Move the value of the working register (rcx) into the return register (rcx)
    // `mov %rcx, %rax`
    // Return
    // `ret`
    machine_code.extend_from_slice(&[0x48, 0x89, 0xc8, 0xc3]);
    machine_code
}

/// Execute a sequence of bytes as x86_64 machine code
/// Expect code to be of the form of a C function with type `int64_t f()`
/// Returns the return value of the passed function
fn run(machine_code: &[u8]) -> i64 {
    // In all probability, this function should be considered unsafe.
    // An arbitrary string of bytes is not guaranteed to be valid x86_64 machine code,
    // Neither is it guaranteed to follow the calling convention used.

    // Rust doesn't have a stable ABI. It's safe to assume the calling convention
    // used with C functions won't change. We'll use that instead.
    type Executable = unsafe extern "C" fn() -> i64;

    let code_len = machine_code.len();

    // Memory allocated by a structure like Vec<u8> is almost certainly not executable.
    // Thus, we can't simply interpret the machine_code slice as a function and run it.
    // First: allocate executable memory
    let memory = region::alloc(code_len, Protection::READ_WRITE_EXECUTE).unwrap();

    let slice =
        unsafe { std::slice::from_raw_parts_mut(memory.as_ptr::<u8>() as *mut u8, memory.len()) };

    // Then: copy the data in machine_code into the memory
    // This is essentially copying a function from non-executable memory to executable memory.
    slice[..code_len].copy_from_slice(machine_code);

    unsafe {
        let ptr = slice.as_ptr();
        let f: Executable = std::mem::transmute(ptr);
        f()
    }
}

#[cfg(test)]
mod test {
    use crate::{jit, run};

    #[test]
    fn test_execution() {
        /// Tester function
        fn t(p: &str) -> i64 {
            run(&jit(p))
        }

        assert_eq!(t("+"), 1);
        assert_eq!(t("++"), 2);
        assert_eq!(t("++/"), 1);
        assert_eq!(t("-"), -1);
        assert_eq!(t("--*"), -4);
        assert_eq!(t("*"), 0);
        assert_eq!(t("/"), 0);
        assert_eq!(t("++*******"), 256);
        assert_eq!(t("--**++"), -6);
    }
}
