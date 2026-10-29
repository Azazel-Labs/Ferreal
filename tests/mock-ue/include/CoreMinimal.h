// A stand-in for the few parts of Unreal Engine that Ferreal's generated C++ uses, so the
// glue can be compiled, linked with the Rust and run without the engine (`cargo xtask
// mock-ue`). It imitates behavior only as far as the tests need. Not Unreal's code.

#pragma once

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstddef>
#include <functional>
#include <map>
#include <memory>
#include <string>
#include <utility>
#include <vector>

using int32 = int32_t;
using int64 = int64_t;
using uint8 = uint8_t;
using uint16 = uint16_t;
using uint32 = uint32_t;
using uint64 = uint64_t;
using ANSICHAR = char;
using TCHAR = char16_t;

#define TEXT(x) u##x
#define PRAGMA_DISABLE_DEPRECATION_WARNINGS
#define PRAGMA_ENABLE_DEPRECATION_WARNINGS
#define UCLASS(...)
#define UENUM(...)
#define UPROPERTY(...)
#define UFUNCTION(...)
#define UMETA(...)
#ifndef WITH_EDITOR
#define WITH_EDITOR 1
#endif

template <typename T>
decltype(auto) MoveTemp(T&& V) { return std::move(V); }

struct FMath
{
	template <typename T> static T Max(T A, T B) { return A < B ? B : A; }
};

template <typename T, typename Alloc = void> class TArray;

class FString
{
public:
	FString() = default;
	FString(const TCHAR* S) : Str(S) {}
	explicit FString(std::u16string S) : Str(std::move(S)) {}
	const TCHAR* operator*() const { return Str.c_str(); }
	int32 Len() const { return static_cast<int32>(Str.size()); }
	void AppendChars(const TCHAR* S, int32 Count) { Str.append(S, static_cast<size_t>(Count)); }
	bool operator==(const FString& O) const { return Str == O.Str; }
	bool operator==(const TCHAR* O) const { return Str == O; }
	bool operator!=(const TCHAR* O) const { return Str != O; }
	inline int32 ParseIntoArray(TArray<FString>& Out, const TCHAR* Delim) const;
	bool Split(const FString& At, FString* Left, FString* Right) const
	{
		const size_t Found = Str.find(At.Str);
		if (Found == std::u16string::npos) return false;
		*Left = FString(Str.substr(0, Found));
		*Right = FString(Str.substr(Found + At.Str.size()));
		return true;
	}

	/** printf with %s (TCHAR strings) and %d, as far as the glue uses it. */
	template <typename... A>
	static FString Printf(const TCHAR* Format, A... Args)
	{
		std::vector<std::u16string> Parts{Arg(Args)...};
		std::u16string Out;
		size_t Next = 0;
		for (const TCHAR* C = Format; *C; ++C)
		{
			if (C[0] == u'%' && (C[1] == u's' || C[1] == u'd'))
			{
				if (Next < Parts.size()) Out += Parts[Next++];
				++C;
			}
			else
			{
				Out += *C;
			}
		}
		return FString(Out);
	}

	std::u16string Str;

private:
	static std::u16string Arg(const TCHAR* S) { return S ? std::u16string(S) : std::u16string(u"(null)"); }
	template <typename N> static std::u16string Arg(N V)
	{
		const std::string D = std::to_string(V);
		return std::u16string(D.begin(), D.end());
	}
};

/** UTF-16 to UTF-8, for test output. */
inline std::string ToUtf8(const FString& S)
{
	std::string Out;
	const std::u16string& U = S.Str;
	for (size_t I = 0; I < U.size(); ++I)
	{
		uint32 C = U[I];
		if (C >= 0xD800 && C < 0xDC00 && I + 1 < U.size())
		{
			C = 0x10000 + ((C - 0xD800) << 10) + (U[++I] - 0xDC00);
		}
		if (C < 0x80) { Out += char(C); }
		else if (C < 0x800) { Out += char(0xC0 | (C >> 6)); Out += char(0x80 | (C & 63)); }
		else if (C < 0x10000) { Out += char(0xE0 | (C >> 12)); Out += char(0x80 | ((C >> 6) & 63)); Out += char(0x80 | (C & 63)); }
		else { Out += char(0xF0 | (C >> 18)); Out += char(0x80 | ((C >> 12) & 63)); Out += char(0x80 | ((C >> 6) & 63)); Out += char(0x80 | (C & 63)); }
	}
	return Out;
}

/** UTF-8 to UTF-16 with an explicit length, as Unreal's FUTF8ToTCHAR. */
class FUTF8ToTCHAR
{
public:
	FUTF8ToTCHAR(const ANSICHAR* S, int32 Len)
	{
		const unsigned char* P = reinterpret_cast<const unsigned char*>(S);
		for (int32 I = 0; I < Len;)
		{
			uint32 C = P[I];
			int N = C < 0x80 ? 1 : C < 0xE0 ? 2 : C < 0xF0 ? 3 : 4;
			if (N == 2) C &= 31; else if (N == 3) C &= 15; else if (N == 4) C &= 7;
			for (int K = 1; K < N && I + K < Len; ++K) C = (C << 6) | (P[I + K] & 63);
			I += N;
			if (C >= 0x10000) { C -= 0x10000; Out += char16_t(0xD800 + (C >> 10)); Out += char16_t(0xDC00 + (C & 1023)); }
			else Out += char16_t(C);
		}
	}
	const TCHAR* Get() const { return Out.c_str(); }
	int32 Length() const { return static_cast<int32>(Out.size()); }
private:
	std::u16string Out;
};

class FName
{
public:
	FName() = default;
	FName(const char* S) : Str(S) {}
	FName(const TCHAR* S) : Str(ToUtf8(FString(S))) {}
	bool operator==(const FName& O) const { return Str == O.Str; }
	FString ToString() const
	{
		FString Out;
		for (char C : Str) Out.Str += char16_t(C);
		return Out;
	}
	std::string Str;
};

#define GET_MEMBER_NAME_CHECKED(Class, Member) ((void)sizeof(&Class::Member), FName(#Member))

template <int N> struct TInlineAllocator {};

template <typename T, typename Alloc>
class TArray
{
public:
	TArray() = default;
	TArray(std::initializer_list<T> L) : V(L) {}
	template <typename U> void Add(U&& E) { V.emplace_back(std::forward<U>(E)); }
	int32 Num() const { return static_cast<int32>(V.size()); }
	T* GetData() { return V.data(); }
	const T* GetData() const { return V.data(); }
	void Reset() { V.clear(); }
	bool Contains(const T& E) const { return std::find(V.begin(), V.end(), E) != V.end(); }
	void Reserve(int32 N) { V.reserve(static_cast<size_t>(N)); }
	T& operator[](int32 I) { return V[static_cast<size_t>(I)]; }
	const T& operator[](int32 I) const { return V[static_cast<size_t>(I)]; }
	auto begin() { return V.begin(); }
	auto end() { return V.end(); }
	auto begin() const { return V.begin(); }
	auto end() const { return V.end(); }
	std::vector<T> V;
};

inline int32 FString::ParseIntoArray(TArray<FString>& Out, const TCHAR* Delim) const
{
	const std::u16string D(Delim);
	size_t At = 0;
	while (true)
	{
		const size_t Found = Str.find(D, At);
		const std::u16string Part = Str.substr(At, Found == std::u16string::npos ? std::u16string::npos : Found - At);
		if (!Part.empty()) Out.Add(FString(Part));
		if (Found == std::u16string::npos) break;
		At = Found + D.size();
	}
	return Out.Num();
}

template <typename T>
class TArrayView
{
public:
	TArrayView(T* InData, int32 InNum) : Data(InData), Count(InNum) {}
	T* GetData() const { return Data; }
	int32 Num() const { return Count; }
	T* begin() const { return Data; }
	T* end() const { return Data + Count; }
private:
	T* Data;
	int32 Count;
};

template <typename T>
using TConstArrayView = TArrayView<const T>;

struct FLinearColor
{
	FLinearColor() = default;
	FLinearColor(float InR, float InG, float InB, float InA = 1.0f) : R(InR), G(InG), B(InB), A(InA) {}
	float R = 0, G = 0, B = 0, A = 0;
};

struct FVector2D
{
	FVector2D() = default;
	FVector2D(double InX, double InY) : X(InX), Y(InY) {}
	double X = 0, Y = 0;
};

struct FVector
{
	FVector() = default;
	FVector(double InX, double InY, double InZ) : X(InX), Y(InY), Z(InZ) {}
	double X = 0, Y = 0, Z = 0;
};

struct FRotator
{
	FRotator() = default;
	FRotator(double InPitch, double InYaw, double InRoll) : Pitch(InPitch), Yaw(InYaw), Roll(InRoll) {}
	double Pitch = 0, Yaw = 0, Roll = 0;
};

struct FQuat
{
	FQuat() = default;
	FQuat(double InX, double InY, double InZ, double InW) : X(InX), Y(InY), Z(InZ), W(InW) {}
	double X = 0, Y = 0, Z = 0, W = 1;
};

/** As the engine's: its members are not public. */
struct FTransform
{
	FTransform() = default;
	FTransform(const FQuat& InRotation, const FVector& InTranslation, const FVector& InScale)
		: Rotation(InRotation), Translation(InTranslation), Scale3D(InScale) {}
	FQuat GetRotation() const { return Rotation; }
	FVector GetTranslation() const { return Translation; }
	FVector GetScale3D() const { return Scale3D; }
protected:
	FQuat Rotation;
	FVector Translation;
	FVector Scale3D{1, 1, 1};
};

/** What UE_LOG printed (the last line, and all of them), for the tests. */
inline FString GFerrealMockLastLog;
inline std::vector<std::string> GFerrealMockLogs;
inline void FerrealMockLog(const char* Verbosity, const TCHAR* Text)
{
	GFerrealMockLastLog = FString(Text);
	GFerrealMockLogs.push_back(ToUtf8(GFerrealMockLastLog));
	std::printf("[%s] %s\n", Verbosity, GFerrealMockLogs.back().c_str());
}
#define DEFINE_LOG_CATEGORY_STATIC(Name, Default, Compiled) [[maybe_unused]] static int Name = 0;
#define UE_LOG(Category, Verbosity, Format, ...) ((void)Category, FerrealMockLog(#Verbosity, *FString::Printf(Format, ##__VA_ARGS__)))

struct FObjectInitializer {};

struct FPropertyChangedEvent
{
	FName Member;
	FName GetMemberPropertyName() const { return Member; }
};

// Reflection, as far as calling functions through ProcessEvent goes: classes found by path,
// their functions, each function's parameters as properties laid out the way UFunction's
// linking does (each aligned, ParmsSize at the end of the last one), and natives that read
// their parameters through those offsets, as the engine's thunks do.

class UObject;

enum EPropertyFlags : uint64
{
	CPF_None = 0,
	CPF_Parm = 0x80,
	CPF_OutParm = 0x100,
	CPF_ReturnParm = 0x400,
};

class FFieldClass
{
public:
	explicit FFieldClass(std::string InName) : Name(std::move(InName)) {}
	FString GetName() const { return FName(Name.c_str()).ToString(); }
	std::string Name;
};

class UScriptStruct
{
public:
	explicit UScriptStruct(std::string InName) : Name(std::move(InName)) {}
	FString GetName() const { return FName(Name.c_str()).ToString(); }
	std::string Name;
};

class FProperty
{
public:
	FProperty(std::string InName, std::string InClass, int32 InSize, int32 InAlign, uint64 InFlags)
		: Name(std::move(InName)), Class(std::move(InClass)), Size(InSize), Align(InAlign), Flags(InFlags) {}
	virtual ~FProperty() = default;
	bool HasAnyPropertyFlags(uint64 F) const { return (Flags & F) != 0; }
	FString GetName() const { return FName(Name.c_str()).ToString(); }
	int32 GetOffset_ForUFunction() const { return Offset; }
	int32 GetSize() const { return Size; }
	const FFieldClass* GetClass() const { return &Class; }
	std::string Name;
	FFieldClass Class;
	int32 Size;
	int32 Align;
	uint64 Flags;
	int32 Offset = 0;
};

class FStructProperty : public FProperty
{
public:
	FStructProperty(std::string InName, UScriptStruct* InStruct, int32 InSize, int32 InAlign, uint64 InFlags)
		: FProperty(std::move(InName), "StructProperty", InSize, InAlign, InFlags), Struct(InStruct) {}
	UScriptStruct* Struct;
};

class FArrayProperty : public FProperty
{
public:
	FArrayProperty(std::string InName, FProperty* InInner, int32 InSize, int32 InAlign, uint64 InFlags)
		: FProperty(std::move(InName), "ArrayProperty", InSize, InAlign, InFlags), Inner(InInner) {}
	~FArrayProperty() override { delete Inner; }
	FProperty* Inner;
};

template <typename T> T* CastField(FProperty* P) { return dynamic_cast<T*>(P); }
template <typename T> const T* CastField(const FProperty* P) { return dynamic_cast<const T*>(P); }

class UFunction
{
public:
	std::string Name;
	std::vector<std::unique_ptr<FProperty>> Properties;
	int32 ParmsSize = 0;
	std::function<void(UObject*, uint8*)> Native;

	/** Lays the parameters out as the engine does. */
	void Link()
	{
		int32 Offset = 0;
		for (auto& P : Properties)
		{
			Offset = (Offset + P->Align - 1) / P->Align * P->Align;
			P->Offset = Offset;
			Offset += P->Size;
			ParmsSize = Offset;
		}
	}

	template <typename T> T& Param(uint8* Parms, int32 Index) { return *reinterpret_cast<T*>(Parms + Properties[static_cast<size_t>(Index)]->Offset); }
};

template <typename T>
class TFieldIterator
{
public:
	explicit TFieldIterator(UFunction* F) : Function(F) {}
	explicit operator bool() const { return Function && Index < Function->Properties.size(); }
	TFieldIterator& operator++() { ++Index; return *this; }
	T* operator*() const { return Function->Properties[Index].get(); }
	T* operator->() const { return Function->Properties[Index].get(); }
private:
	UFunction* Function;
	size_t Index = 0;
};

class UClass
{
public:
	UClass(std::string InName, UClass* InSuper) : Name(std::move(InName)), Super(InSuper) {}
	UFunction* FindFunctionByName(const FName& N) const
	{
		for (const UClass* C = this; C; C = C->Super)
		{
			auto It = C->Functions.find(N.Str);
			if (It != C->Functions.end()) return It->second.get();
		}
		return nullptr;
	}
	bool IsChildOf(const UClass* Other) const
	{
		for (const UClass* C = this; C; C = C->Super)
		{
			if (C == Other) return true;
		}
		return false;
	}
	UObject* GetDefaultObject() const { return DefaultObject; }
	FString GetName() const { return FName(Name.c_str()).ToString(); }
	std::string Name;
	UClass* Super;
	std::map<std::string, std::unique_ptr<UFunction>> Functions;
	UObject* DefaultObject = nullptr;
};

/** The classes FindObject finds, by path. */
inline std::map<std::u16string, UClass*>& FerrealMockClasses()
{
	static std::map<std::u16string, UClass*> Classes;
	return Classes;
}

template <typename T>
T* FindObject(UObject*, const TCHAR* Path)
{
	auto It = FerrealMockClasses().find(Path);
	return It == FerrealMockClasses().end() ? nullptr : It->second;
}

class UObject
{
public:
	UObject() = default;
	explicit UObject(const FObjectInitializer&) {}
	virtual ~UObject() = default;
	virtual void BeginDestroy() {}
	virtual void PostEditChangeProperty(FPropertyChangedEvent&) {}

	UClass* GetClass() const { return MockClass; }
	bool IsA(const UClass* C) const { return MockClass && MockClass->IsChildOf(C); }
	UFunction* FindFunction(const FName& N) const { return MockClass ? MockClass->FindFunctionByName(N) : nullptr; }
	void ProcessEvent(UFunction* F, void* Parms) { F->Native(this, static_cast<uint8*>(Parms)); }
	FString GetName() const { return FName(MockName.c_str()).ToString(); }

	// Not Unreal's: the mock's (names that the generated classes' properties cannot clash with).
	UClass* MockClass = nullptr;
	std::string MockName = "Object";
};

inline bool IsValid(const UObject* O) { return O != nullptr; }

template <typename T>
class TObjectPtr
{
public:
	TObjectPtr() = default;
	TObjectPtr(T* P) : Ptr(P) {}
	T* Get() const { return Ptr; }
	operator T*() const { return Ptr; }
	T* operator->() const { return Ptr; }
private:
	T* Ptr = nullptr;
};

template <typename T>
T* Cast(UObject* O) { return dynamic_cast<T*>(O); }

/** A weak pointer, without the weakness: what the glue reads and writes. */
template <typename T>
class TWeakObjectPtr
{
public:
	TWeakObjectPtr() = default;
	TWeakObjectPtr(T* O) : Ptr(O) {}
	T* Get() const { return Ptr; }
private:
	T* Ptr = nullptr;
};
