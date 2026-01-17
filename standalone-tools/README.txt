===============================================================================
   CAN LOG READER - BLF FILE ANALYZER (Standalone Tool)
===============================================================================

WHAT IS THIS?
-------------
This tool analyzes BLF (Binary Log Format) files from Vector CANoe/CANalyzer.
It tells you what types of CAN messages your file contains and whether they're
supported by the parser.

HOW TO USE:
-----------
1. Open Command Prompt (cmd.exe) or PowerShell

2. Navigate to this folder:
   cd "C:\path\to\standalone-tools"

3. Run the analyzer on your BLF file:
   analyze_blf.exe "C:\path\to\your\logfile.blf"

EXAMPLE:
--------
analyze_blf.exe "C:\logs\production_run_2024_01_15.blf"

WHAT IT SHOWS:
--------------
- File size and validation status
- Object type statistics (what's in the file)
- Compression detection
- Whether your file is supported
- Recommendations for next steps

INTERPRETING RESULTS:
---------------------
If you see:
  ✅ "Type 86" messages → File is FULLY SUPPORTED
  ⚠️  "Type 100/101" messages → Needs additional work (options available)

SUPPORTED FILE TYPES:
---------------------
✅ Type 86 (CanMessage2) - Standard CAN and CAN-FD messages
✅ Type 73 (CanErrorFrameExt) - CAN error frames
⚠️  Type 100 (CAN_FD_MESSAGE) - Not yet supported (can be converted)
⚠️  Type 101 (CAN_FD_MESSAGE_64) - Not yet supported (can be converted)

TROUBLESHOOTING:
----------------
If the tool doesn't run:
1. Make sure you're using Windows x64
2. Try running from Command Prompt (not double-clicking)
3. Check that the path to your BLF file is correct

If you get "file not found":
- Use full paths with quotes
- Example: analyze_blf.exe "C:\Users\YourName\Documents\logfile.blf"

NEXT STEPS:
-----------
After analyzing your file:

1. If file is Type 86:
   - You're ready! The parser fully supports your files
   - Report back: "My files use Type 86, ready to proceed"

2. If file is Type 100/101:
   - Option A: Re-export from CANoe with Type 86 format (recommended)
   - Option B: Wait for Type 100/101 parser implementation
   - Option C: Use MF4 format instead

3. Save the output and share it for further guidance

TECHNICAL DETAILS:
------------------
- Executable size: ~299 KB
- No installation needed (standalone)
- No dependencies required
- Built with Rust (optimized release build)
- Uses 'ablf' crate v0.2.0 for BLF parsing

REPORTING ISSUES:
-----------------
If you encounter problems:
1. Run the analyzer on your file
2. Save the complete output (copy from terminal)
3. Create a GitHub issue at:
   https://github.com/MartinGrozev/CAN-log-reader-Rust.git
4. Include:
   - The analyzer output
   - File size
   - Where the file came from (CANoe version, etc.)

VERSION:
--------
Built: 2026-01-17
Session: 8
Commit: 9d2b8f1

===============================================================================
For full documentation see QUICKSTART.md in the repository
===============================================================================
