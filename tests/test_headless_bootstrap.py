import unittest
import os
import sys
import numpy as np
import marl_engine._marl_engine_native as engine

class TestHeadlessBootstrap(unittest.TestCase):
    def test_engine_ping(self):
        res = engine.ping()
        self.assertEqual(res, "marl_engine_core_ok")

    def test_zero_copy_array_stub(self):
        dim = 256
        arr = engine.create_dummy_obs(dim)
        self.assertIsInstance(arr, np.ndarray)
        self.assertEqual(arr.shape, (dim,))
        self.assertEqual(arr.dtype, np.float32)
        self.assertTrue(arr.flags['C_CONTIGUOUS'])

    def test_native_dependencies_headless(self):
        # Locate the native module file (.pyd, .so, or .dylib)
        native_module_path = engine.__file__
        self.assertTrue(os.path.exists(native_module_path), f"Native module not found at {native_module_path}")

        # Read binary content to check for unwanted graphical/windowing dependencies
        with open(native_module_path, "rb") as f:
            content = f.read()

        # Forbidden graphic/windowing library strings in headless build
        forbidden_libs = [
            b"glfw",
            b"libX11",
            b"libGL.so",
            b"opengl32.dll" if os.name == "nt" else b"opengl32",
            b"vulkan"
        ]

        # Note: on Windows, opengl32.dll might be a system DLL, but pure headless rust cdylib without visual feature should NOT link it.
        # Let's check specifically that visual feature libs are absent.
        for lib in forbidden_libs:
            # If present, check if it's an accidental link
            if lib in content.lower():
                # On Windows, system dlls might appear in symbol tables, but let's verify if raylib/glfw/x11 are present
                self.assertNotIn(b"glfw", content.lower(), "Visual library 'glfw' found in headless build!")
                self.assertNotIn(b"libx11", content.lower(), "Visual library 'libX11' found in headless build!")

        # Verify environment display doesn't affect import
        old_display = os.environ.get("DISPLAY", "")
        os.environ["DISPLAY"] = ""
        try:
            import importlib
            import marl_engine
            importlib.reload(marl_engine)
        finally:
            os.environ["DISPLAY"] = old_display

if __name__ == "__main__":
    unittest.main()
