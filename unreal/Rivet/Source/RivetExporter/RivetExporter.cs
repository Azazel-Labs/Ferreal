// Rivet's exporter for Unreal Header Tool: writes the reflection data of every class and enum
// UHT parsed (the engine's, the project's, its plugins') into one JSON file, which
// rivet-build reads to generate typed Rust bindings of the engine classes a crate names.
//
// It writes what calling a function through reflection needs, and nothing about memory
// layout: the classes with their path and parent, their reflected functions with flags and
// parameters (each parameter's kind, the engine property class that will hold it, its
// flags), and the enums with their values. rivet-build lays out each call's parameters as a
// C++ struct, and the generated code checks that struct against the engine's own reflection
// when it first calls the function.
//
// Output: Intermediate/Build/<Platform>/<Target>/Inc/RivetMetadata/UHT/RivetEngine.json, in
// the Rivet plugin (the RivetMetadata module is the output's home).

using System.Text;
using System.Text.Encodings.Web;
using System.Text.Json;
using EpicGames.Core;
using EpicGames.UHT.Tables;
using EpicGames.UHT.Types;
using EpicGames.UHT.Utils;

namespace Rivet;

[UnrealHeaderTool]
public static class RivetExporter
{
	/// <summary>The format rivet-build expects (rivet-build/src/engine.rs, FORMAT_VERSION).</summary>
	private const int FormatVersion = 1;

	[UhtExporter(
		Name = "Rivet",
		Description = "Reflection data for Rivet's Rust bindings",
		Options = UhtExporterOptions.Default,
		ModuleName = "RivetMetadata")]
	public static void Export(IUhtExportFactory factory)
	{
		using var stream = new MemoryStream();
		var options = new JsonWriterOptions
		{
			Indented = false,
			Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
		};
		using (var json = new Utf8JsonWriter(stream, options))
		{
			json.WriteStartObject();
			json.WriteString("format", "rivet-engine");
			json.WriteNumber("version", FormatVersion);

			var enums = new List<(UhtEnum Enum, string Module)>();
			json.WriteStartArray("classes");
			foreach (UhtModule module in factory.Session.Modules)
			{
				foreach (UhtHeaderFile header in module.Headers)
				{
					foreach (UhtType type in header.Children)
					{
						switch (type)
						{
							case UhtClass c when c.ClassType == UhtClassType.Class:
								WriteClass(json, c, module.ShortName, header);
								break;
							case UhtEnum e:
								enums.Add((e, module.ShortName));
								break;
						}
					}
				}
			}
			json.WriteEndArray();

			json.WriteStartArray("enums");
			foreach ((UhtEnum e, string module) in enums)
			{
				WriteEnum(json, e, module);
			}
			json.WriteEndArray();

			json.WriteEndObject();
		}

		string path = factory.MakePath("RivetEngine", ".json");
		factory.CommitOutput(path, new StringBuilder(Encoding.UTF8.GetString(stream.ToArray())));
	}

	private static void WriteClass(Utf8JsonWriter json, UhtClass c, string module, UhtHeaderFile header)
	{
		json.WriteStartObject();
		json.WriteString("cpp", c.SourceName);
		json.WriteString("path", $"/Script/{module}.{c.EngineName}");
		json.WriteString("module", module);
		string include = header.IncludeFilePath ?? header.ModuleRelativeFilePath;
		if (!string.IsNullOrEmpty(include))
		{
			json.WriteString("header", include);
		}
		if (c.SuperClass != null)
		{
			json.WriteString("super", c.SuperClass.SourceName);
		}
		json.WriteBoolean("abstract", c.ClassFlags.HasAnyFlags(EClassFlags.Abstract));
		json.WriteBoolean("deprecated", c.ClassFlags.HasAnyFlags(EClassFlags.Deprecated));
		WriteDoc(json, c);

		json.WriteStartArray("functions");
		foreach (UhtFunction f in c.Functions)
		{
			if (Exported(f))
			{
				WriteFunction(json, f);
			}
		}
		json.WriteEndArray();
		json.WriteEndObject();
	}

	/// <summary>Functions reflection can call: not delegates or network messages, not private.</summary>
	private static bool Exported(UhtFunction f)
	{
		if (f.FunctionType != UhtFunctionType.Function)
		{
			return false;
		}
		if (f.FunctionFlags.HasAnyFlags(EFunctionFlags.Delegate | EFunctionFlags.Private | EFunctionFlags.NetRequest | EFunctionFlags.NetResponse))
		{
			return false;
		}
		return f.FunctionFlags.HasAnyFlags(EFunctionFlags.Native | EFunctionFlags.BlueprintEvent);
	}

	private static void WriteFunction(Utf8JsonWriter json, UhtFunction f)
	{
		EFunctionFlags flags = f.FunctionFlags;
		json.WriteStartObject();
		json.WriteString("cpp", f.SourceName);
		json.WriteString("name", f.EngineName);
		json.WriteBoolean("static", flags.HasAnyFlags(EFunctionFlags.Static));
		json.WriteBoolean("const", flags.HasAnyFlags(EFunctionFlags.Const));
		json.WriteBoolean("callable", flags.HasAnyFlags(EFunctionFlags.BlueprintCallable));
		json.WriteBoolean("pure", flags.HasAnyFlags(EFunctionFlags.BlueprintPure));
		json.WriteBoolean("event", flags.HasAnyFlags(EFunctionFlags.BlueprintEvent));
		json.WriteBoolean("protected", flags.HasAnyFlags(EFunctionFlags.Protected));
		json.WriteBoolean("net", flags.HasAnyFlags(EFunctionFlags.Net));
		json.WriteBoolean("editor_only", flags.HasAnyFlags(EFunctionFlags.EditorOnly));
		json.WriteBoolean("internal", f.MetaData.ContainsKey("BlueprintInternalUseOnly"));
		if (f.MetaData.TryGetValue("DeprecationMessage", out string message))
		{
			json.WriteString("deprecated", message);
		}
		else if (f.MetaData.ContainsKey("DeprecatedFunction"))
		{
			json.WriteString("deprecated", "");
		}
		WriteDoc(json, f);

		json.WriteStartArray("params");
		foreach (UhtType child in f.ParameterProperties.Span)
		{
			if (child is UhtProperty p)
			{
				WriteParam(json, p, f, false);
			}
		}
		if (f.ReturnProperty != null)
		{
			WriteParam(json, f.ReturnProperty, f, true);
		}
		json.WriteEndArray();
		json.WriteEndObject();
	}

	private static void WriteParam(Utf8JsonWriter json, UhtProperty p, UhtFunction f, bool isReturn)
	{
		EPropertyFlags flags = p.PropertyFlags;
		json.WriteStartObject();
		json.WriteString("name", p.SourceName);
		json.WritePropertyName("type");
		WriteType(json, p);
		json.WriteBoolean("ret", isReturn);
		json.WriteBoolean("out", !isReturn && flags.HasAnyFlags(EPropertyFlags.OutParm));
		json.WriteBoolean("const", flags.HasAnyFlags(EPropertyFlags.ConstParm));
		json.WriteBoolean("ref", flags.HasAnyFlags(EPropertyFlags.ReferenceParm));
		if (!isReturn && f.MetaData.TryGetValue($"CPP_Default_{p.SourceName}", out string value))
		{
			json.WriteString("default", value);
		}
		json.WriteEndObject();
	}

	/// <summary>
	/// A property's type: its kind, and the engine class of its FProperty (what the generated
	/// code checks at run time), with the class, struct, enum or element it refers to.
	/// </summary>
	private static void WriteType(Utf8JsonWriter json, UhtProperty p)
	{
		json.WriteStartObject();
		switch (p)
		{
			case UhtBoolProperty:
				Kind(json, "bool", "BoolProperty");
				break;
			case UhtByteProperty b:
				Kind(json, b.Enum != null ? "enum" : "uint8", "ByteProperty");
				if (b.Enum != null)
				{
					json.WriteString("enum", b.Enum.SourceName);
				}
				break;
			case UhtInt8Property:
				Kind(json, "int8", "Int8Property");
				break;
			case UhtInt16Property:
				Kind(json, "int16", "Int16Property");
				break;
			case UhtIntProperty:
				Kind(json, "int32", "IntProperty");
				break;
			case UhtInt64Property:
				Kind(json, "int64", "Int64Property");
				break;
			case UhtUInt16Property:
				Kind(json, "uint16", "UInt16Property");
				break;
			case UhtUInt32Property:
				Kind(json, "uint32", "UInt32Property");
				break;
			case UhtUInt64Property:
				Kind(json, "uint64", "UInt64Property");
				break;
			case UhtFloatProperty:
				Kind(json, "float", "FloatProperty");
				break;
			case UhtLargeWorldCoordinatesRealProperty:
				Kind(json, "double", "LargeWorldCoordinatesRealProperty");
				break;
			case UhtDoubleProperty:
				Kind(json, "double", "DoubleProperty");
				break;
			case UhtStrProperty:
				Kind(json, "string", "StrProperty");
				break;
			case UhtNameProperty:
				Kind(json, "name", "NameProperty");
				break;
			case UhtTextProperty:
				Kind(json, "text", "TextProperty");
				break;
			case UhtEnumProperty e:
				Kind(json, "enum", "EnumProperty");
				json.WriteString("enum", e.Enum.SourceName);
				break;
			case UhtStructProperty s:
				Kind(json, "struct", "StructProperty");
				json.WriteString("struct", s.ScriptStruct.SourceName);
				break;
			case UhtClassProperty c:
				Kind(json, "class", "ClassProperty");
				json.WriteString("class", c.MetaClass?.SourceName ?? "UObject");
				break;
			case UhtSoftClassProperty:
				Kind(json, "other", "SoftClassProperty");
				break;
			case UhtSoftObjectProperty:
				Kind(json, "other", "SoftObjectProperty");
				break;
			case UhtWeakObjectPtrProperty:
				Kind(json, "other", "WeakObjectProperty");
				break;
			case UhtLazyObjectPtrProperty:
				Kind(json, "other", "LazyObjectProperty");
				break;
			case UhtInterfaceProperty:
				Kind(json, "other", "InterfaceProperty");
				break;
			case UhtObjectProperty o:
				// A raw pointer or a TObjectPtr: a pointer either way.
				Kind(json, "object", "ObjectProperty");
				json.WriteString("class", o.Class.SourceName);
				break;
			case UhtArrayProperty a:
				Kind(json, "array", "ArrayProperty");
				json.WritePropertyName("inner");
				WriteType(json, a.ValueProperty);
				break;
			default:
				Kind(json, "other", p.EngineClassName);
				break;
		}
		json.WriteEndObject();
	}

	private static void Kind(Utf8JsonWriter json, string kind, string property)
	{
		json.WriteString("kind", kind);
		json.WriteString("property", property);
	}

	private static void WriteEnum(Utf8JsonWriter json, UhtEnum e, string module)
	{
		json.WriteStartObject();
		json.WriteString("cpp", e.SourceName);
		json.WriteString("path", $"/Script/{module}.{e.EngineName}");
		json.WriteString("underlying", e.UnderlyingType switch
		{
			UhtEnumUnderlyingType.Uint8 => "uint8",
			UhtEnumUnderlyingType.Int8 => "int8",
			UhtEnumUnderlyingType.Int16 => "int16",
			UhtEnumUnderlyingType.Uint16 => "uint16",
			UhtEnumUnderlyingType.Int32 => "int32",
			UhtEnumUnderlyingType.Uint32 => "uint32",
			UhtEnumUnderlyingType.Int64 => "int64",
			UhtEnumUnderlyingType.Uint64 => "uint64",
			_ => "unspecified",
		});
		WriteDoc(json, e);
		json.WriteStartArray("values");
		foreach (UhtEnumValue v in e.EnumValues)
		{
			// "EFoo::Bar" and "Bar" alike: the name after the last "::". UHT records -1 for a
			// value it could not evaluate (an expression); rivet-build leaves those out.
			string name = v.Name;
			int colon = name.LastIndexOf("::", StringComparison.Ordinal);
			if (colon >= 0)
			{
				name = name[(colon + 2)..];
			}
			json.WriteStartObject();
			json.WriteString("name", name);
			json.WriteNumber("value", v.Value);
			json.WriteEndObject();
		}
		json.WriteEndArray();
		json.WriteEndObject();
	}

	private static void WriteDoc(Utf8JsonWriter json, UhtType type)
	{
		if (type.MetaData.TryGetValue("ToolTip", out string tip) && !string.IsNullOrWhiteSpace(tip))
		{
			json.WriteString("doc", tip);
		}
	}
}
