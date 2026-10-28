// A stand-in for the parts of EpicGames.Core and EpicGames.UHT the exporter uses, shaped as
// the exporter uses them (the real assemblies come with an engine): it checks the exporter's
// syntax and logic, not that UHT's API matches.
using System.Text;
namespace EpicGames.Core
{
	[Flags] public enum EClassFlags : uint { None = 0, Abstract = 1, Deprecated = 2 }
	[Flags] public enum EFunctionFlags : uint { None = 0, Static = 1, Const = 2, BlueprintCallable = 4, BlueprintPure = 8, BlueprintEvent = 16, Protected = 32, Private = 64, Net = 128, NetRequest = 256, NetResponse = 512, Delegate = 1024, Native = 2048, EditorOnly = 4096, Public = 8192 }
	[Flags] public enum EPropertyFlags : ulong { None = 0, OutParm = 1, ConstParm = 2, ReferenceParm = 4, Deprecated = 0x20000000, EditorOnly = 0x800000000, NativeAccessSpecifierPublic = 0x0010000000000000, NativeAccessSpecifierProtected = 0x0020000000000000, NativeAccessSpecifierPrivate = 0x0040000000000000 }
	[Flags] public enum EStructFlags : uint { NoFlags = 0, NoExport = 8 }
	public static class FlagsExtensions
	{
		public static bool HasAnyFlags(this EClassFlags a, EClassFlags b) => (a & b) != 0;
		public static bool HasAnyFlags(this EFunctionFlags a, EFunctionFlags b) => (a & b) != 0;
		public static bool HasAnyFlags(this EPropertyFlags a, EPropertyFlags b) => (a & b) != 0;
		public static bool HasAnyFlags(this EStructFlags a, EStructFlags b) => (a & b) != 0;
	}
}
namespace EpicGames.UHT.Tables
{
	using EpicGames.UHT.Utils;
	[AttributeUsage(AttributeTargets.Class)] public sealed class UnrealHeaderToolAttribute : Attribute { }
	[Flags] public enum UhtExporterOptions { None = 0, Default = 1 }
	[AttributeUsage(AttributeTargets.Method)] public sealed class UhtExporterAttribute : Attribute
	{
		public string Name { get; set; } public string Description { get; set; } public UhtExporterOptions Options { get; set; } public string ModuleName { get; set; }
	}
}
namespace EpicGames.UHT.Types
{
	public class UhtMetaData : Dictionary<string, string> { }
	public class UhtType { public string SourceName = ""; public string EngineName = ""; public string EngineClassName = ""; public UhtMetaData MetaData = new(); public List<UhtType> Children = new(); }
	public enum UhtClassType { Class, Interface, NativeInterface }
	public class UhtClass : UhtType { public UhtClassType ClassType; public UhtClass SuperClass; public EpicGames.Core.EClassFlags ClassFlags; public List<UhtFunction> Functions = new(); }
	public enum UhtFunctionType { Function, Delegate }
	public class UhtFunction : UhtType { public UhtFunctionType FunctionType; public EpicGames.Core.EFunctionFlags FunctionFlags; public ReadOnlyMemory<UhtType> ParameterProperties = ReadOnlyMemory<UhtType>.Empty; public UhtProperty ReturnProperty; }
	public class UhtProperty : UhtType { public EpicGames.Core.EPropertyFlags PropertyFlags; public string ArrayDimensions; }
	public class UhtBoolProperty : UhtProperty { }
	public class UhtByteProperty : UhtProperty { public UhtEnum Enum; }
	public class UhtInt8Property : UhtProperty { } public class UhtInt16Property : UhtProperty { } public class UhtIntProperty : UhtProperty { } public class UhtInt64Property : UhtProperty { }
	public class UhtUInt16Property : UhtProperty { } public class UhtUInt32Property : UhtProperty { } public class UhtUInt64Property : UhtProperty { }
	public class UhtFloatProperty : UhtProperty { } public class UhtDoubleProperty : UhtProperty { } public class UhtLargeWorldCoordinatesRealProperty : UhtDoubleProperty { }
	public class UhtStrProperty : UhtProperty { } public class UhtNameProperty : UhtProperty { } public class UhtTextProperty : UhtProperty { }
	public class UhtEnumProperty : UhtProperty { public UhtEnum Enum; }
	public class UhtScriptStruct : UhtType { public EpicGames.Core.EStructFlags ScriptStructFlags; public UhtScriptStruct SuperScriptStruct; public IEnumerable<UhtProperty> Properties => Children.OfType<UhtProperty>(); }
	public class UhtStructProperty : UhtProperty { public UhtScriptStruct ScriptStruct; }
	public class UhtObjectPropertyBase : UhtProperty { public UhtClass Class; }
	public class UhtObjectProperty : UhtObjectPropertyBase { }
	public class UhtClassProperty : UhtObjectProperty { public UhtClass MetaClass; }
	public class UhtSoftObjectProperty : UhtObjectPropertyBase { } public class UhtSoftClassProperty : UhtSoftObjectProperty { }
	public class UhtWeakObjectPtrProperty : UhtObjectPropertyBase { } public class UhtLazyObjectPtrProperty : UhtObjectPropertyBase { }
	public class UhtInterfaceProperty : UhtProperty { }
	public class UhtArrayProperty : UhtProperty { public UhtProperty ValueProperty; }
	public enum UhtEnumUnderlyingType { Unspecified, Uint8, Uint16, Uint32, Uint64, Int8, Int16, Int32, Int64, Int }
	public class UhtEnumValue { public string Name = ""; public long Value; }
	public class UhtEnum : UhtType { public UhtEnumUnderlyingType UnderlyingType; public List<UhtEnumValue> EnumValues = new(); }
	public class UhtHeaderFile { public List<UhtType> Children = new(); public string IncludeFilePath; public string ModuleRelativeFilePath; }
	public class UhtModule { public string ShortName = ""; public List<UhtHeaderFile> Headers = new(); }
	public class UhtSession { public List<UhtModule> Modules = new(); }
}
namespace EpicGames.UHT.Utils
{
	public interface IUhtExportFactory
	{
		EpicGames.UHT.Types.UhtSession Session { get; }
		string MakePath(string name, string extension);
		void CommitOutput(string path, StringBuilder contents);
	}
}
